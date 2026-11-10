//! Poller de memória do PC: publica `EventKey::HardwareAlert` (fala proativa) e um
//! `EventKey::Notification` (toast) quando a folga de memória PIORA de nível.
//!
//! Nasceu de um crash real (2026-09-27): `memory allocation of 2684514 bytes failed` no dev, com a
//! Mika plana em ~1,3 GB — o commit do Windows inteiro tinha acabado (llama-server + builds em
//! paralelo). Quem morre é quem aloca na hora, não quem gasta; avisar antes é a única defesa.
//!
//! **Folga por SO.** No Windows o limite é o commit (RAM + pagefile): com ele cheio, `HeapAlloc`
//! falha mesmo com RAM física livre. `sysinfo` no Windows deriva swap de `CommitLimit`/`CommitTotal`
//! (`K32GetPerformanceInfo`), então `free_swap` JÁ é a folga de commit sempre que o commit passou da
//! RAM física — o único regime em que o alerta importa. Linux/macOS fazem overcommit e o risco é o
//! OOM killer: folga = disponível + swap livre.
//!
//! Processos (caro de enumerar) só são lidos na escalada, para nomear quem está gastando.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const MIB: u64 = 1024 * 1024;
const TOP_CONSUMERS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Ok,
    Warning,
    Critical,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Ok => "ok",
            Level::Warning => "warning",
            Level::Critical => "critical",
        }
    }
}

/// Puro: bytes que ainda podem ser alocados antes de o SO recusar/matar alguém.
fn headroom_bytes(windows: bool, available: u64, free_swap: u64) -> u64 {
    if windows {
        free_swap
    } else {
        available.saturating_add(free_swap)
    }
}

/// Puro: nível para a folga. Limite é o MAIOR entre o percentual da capacidade e um piso absoluto
/// (máquina pequena não vive a 5% de 8 GB). `scale_pct` > 100 é a histerese de saída.
fn level_for(headroom: u64, capacity: u64, scale_pct: u64) -> Level {
    let below = |pct: u64, floor_mib: u64| {
        let limit = (capacity / 100 * pct).max(floor_mib * MIB);
        headroom < limit / 100 * scale_pct
    };
    if below(5, 768) {
        Level::Critical
    } else if below(10, 1536) {
        Level::Warning
    } else {
        Level::Ok
    }
}

/// Puro: piora vale na hora; melhora só com 50% de folga acima do limite, para a leitura que oscila
/// na borda não gerar um aviso a cada tick.
fn next_level(prev: Level, headroom: u64, capacity: u64) -> Level {
    let raw = level_for(headroom, capacity, 100);
    if raw >= prev {
        raw
    } else {
        level_for(headroom, capacity, 150).min(prev)
    }
}

/// Puro: soma por nome (19 `msedgewebview2` são um programa só para o usuário) e devolve os maiores.
fn top_consumers(entries: impl IntoIterator<Item = (String, u64)>, n: usize) -> Vec<(String, u64)> {
    let mut by_name: HashMap<String, u64> = HashMap::new();
    for (name, bytes) in entries {
        *by_name.entry(name).or_default() += bytes;
    }
    let mut sorted: Vec<_> = by_name.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.truncate(n);
    sorted
}

fn format_gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1024.0 * MIB as f64))
}

/// Puro: o payload do `HardwareAlert` — o que a reação proativa e o toast leem.
fn alert_payload(level: Level, headroom: u64, capacity: u64, top: &[(String, u64)]) -> Value {
    json!({
        "level": level.as_str(),
        "free": format_gb(headroom),
        "total": format_gb(capacity),
        "top": top.iter().map(|(name, bytes)| json!({ "name": name, "memory": format_gb(*bytes) })).collect::<Vec<_>>(),
    })
}

/// Puro: o toast traduzido. Sempre sai, mesmo quando a fala é barrada por cooldown/horário.
fn notice_from(payload: &Value, locale: &str) -> Value {
    let t = |key: &str| crate::modules::settings::t(key, locale);
    let critical = payload["level"] == "critical";
    let top = payload["top"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| Some(format!("{} ({})", item["name"].as_str()?, item["memory"].as_str()?)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let key = if critical { "system.memory.notification.critical" } else { "system.memory.notification.warning" };
    let mut message = t(key)
        .replace("{{free}}", payload["free"].as_str().unwrap_or("?"))
        .replace("{{total}}", payload["total"].as_str().unwrap_or("?"));
    if !top.is_empty() {
        message = format!("{message} {}", t("system.memory.notification.top").replace("{{top}}", &top));
    }
    json!({
        "type": if critical { "error" } else { "warning" },
        "source": t("system.memory.notification.source"),
        "message": message,
    })
}

fn read_headroom(system: &mut System) -> (u64, u64) {
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram().with_swap());
    let capacity = system.total_memory().saturating_add(system.total_swap());
    let headroom = headroom_bytes(cfg!(windows), system.available_memory(), system.free_swap());
    (headroom, capacity)
}

/// Commit privado no Windows (o que consome o limite); RSS nos demais.
fn read_top_consumers(system: &mut System) -> Vec<(String, u64)> {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());
    let entries = system.processes().values().map(|process| {
        let bytes = if cfg!(windows) { process.virtual_memory() } else { process.memory() };
        (process.name().to_string_lossy().into_owned(), bytes)
    });
    top_consumers(entries, TOP_CONSUMERS)
}

fn locale(store: &ConfigStore) -> String {
    crate::config::locale::app_locale(store)
}

/// Sobe o laço. Leitura de memória/processos é síncrona: roda em `spawn_blocking` com o `System`
/// reaproveitado entre ticks (recriar enumeraria tudo de novo).
pub fn spawn_memory_pressure_watcher(bus: &'static EventBus, store: Arc<ConfigStore>) {
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        let mut system = Some(System::new_with_specifics(RefreshKind::nothing()));
        let mut level = Level::Ok;
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(crate::modules::system::governor::background(POLL_INTERVAL)) => {}
            }
            let Some(mut sys) = system.take() else { break };
            let prev = level;
            let joined = tokio::task::spawn_blocking(move || {
                let (headroom, capacity) = read_headroom(&mut sys);
                let next = next_level(prev, headroom, capacity);
                let payload = (next > prev).then(|| alert_payload(next, headroom, capacity, &read_top_consumers(&mut sys)));
                (sys, next, payload)
            })
            .await;
            let Ok((sys, next, payload)) = joined else {
                log::warn!("poller de memória: task de leitura falhou; parando o poller");
                break;
            };
            system = Some(sys);
            level = next;
            if let Some(payload) = payload {
                log::warn!("memória do PC em nível {}: {payload}", next.as_str());
                bus.publish(EventKey::Notification, notice_from(&payload, &locale(&store)));
                bus.publish(EventKey::HardwareAlert, payload);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * MIB;

    #[test]
    fn windows_headroom_is_the_commit_left_not_free_ram() {
        // O crash real: 7,6 GB de RAM livre e só 5,5 GB de commit — a RAM livre não salva ninguém.
        assert_eq!(headroom_bytes(true, 7 * GIB, 5 * GIB), 5 * GIB);
        assert_eq!(headroom_bytes(false, 7 * GIB, 5 * GIB), 12 * GIB);
    }

    #[test]
    fn the_crash_scenario_is_already_a_warning() {
        // 5,5 GB livres de 55,7 GB de commit, minutos antes da alocação falhar.
        assert_eq!(level_for(5_500 * MIB, 55_700 * MIB, 100), Level::Warning);
        assert_eq!(level_for(2 * GIB, 55_700 * MIB, 100), Level::Critical);
        assert_eq!(level_for(20 * GIB, 55_700 * MIB, 100), Level::Ok);
    }

    #[test]
    fn small_machines_use_the_absolute_floor() {
        // 8 GB: 10% seria 800 MB, tarde demais — o piso de 1,5 GB manda.
        assert_eq!(level_for(1200 * MIB, 8 * GIB, 100), Level::Warning);
        assert_eq!(level_for(700 * MIB, 8 * GIB, 100), Level::Critical);
    }

    #[test]
    fn worsening_applies_at_once() {
        assert_eq!(next_level(Level::Ok, 2 * GIB, 55_700 * MIB), Level::Critical);
    }

    #[test]
    fn recovery_needs_margin_so_the_edge_does_not_flap() {
        let capacity = 100 * GIB;
        // 12%: já saiu do limite de 10%, mas não dos 15% da histerese.
        assert_eq!(next_level(Level::Warning, 12 * GIB, capacity), Level::Warning);
        assert_eq!(next_level(Level::Warning, 16 * GIB, capacity), Level::Ok);
        // Crítico a 6% desce para aviso só depois de 7,5%.
        assert_eq!(next_level(Level::Critical, 6 * GIB, capacity), Level::Critical);
        assert_eq!(next_level(Level::Critical, 8 * GIB, capacity), Level::Warning);
    }

    #[test]
    fn consumers_are_summed_per_program_and_ranked() {
        let top = top_consumers(
            vec![
                ("rust-analyzer".to_string(), 2 * GIB),
                ("llama-server".to_string(), 17 * GIB),
                ("rust-analyzer".to_string(), 5 * GIB),
                ("node".to_string(), GIB),
                ("claude".to_string(), 3 * GIB),
            ],
            3,
        );
        let names: Vec<_> = top.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["llama-server", "rust-analyzer", "claude"]);
        assert_eq!(top[1].1, 7 * GIB);
    }

    #[test]
    fn the_notice_names_the_numbers_and_the_culprits_in_the_user_language() {
        let payload = alert_payload(Level::Critical, 2 * GIB, 56 * GIB, &[("llama-server".to_string(), 17 * GIB)]);
        let notice = notice_from(&payload, "pt-BR");
        assert_eq!(notice["type"], "error");
        let message = notice["message"].as_str().expect("texto");
        assert!(message.contains("2.0 GB") && message.contains("56.0 GB"), "{message}");
        assert!(message.contains("llama-server (17.0 GB)"), "{message}");
        assert!(!message.contains("{{"), "placeholder sem substituir: {message}");
        assert_eq!(notice_from(&payload, "en-US")["source"], "PC memory");
    }

    #[test]
    fn a_warning_is_a_warning_toast() {
        let payload = alert_payload(Level::Warning, 5 * GIB, 56 * GIB, &[]);
        assert_eq!(notice_from(&payload, "pt-BR")["type"], "warning");
        assert_eq!(payload["top"], json!([]));
    }
}
