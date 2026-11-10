//! Watcher de janela ativa — porte de `services/brain/awareness/active-window-watcher.ts` +
//! `infrastructure/os/foreground-window.ts`/`foreground-native.ts`: sonda a janela em primeiro
//! plano a cada [`POLL_INTERVAL`] e publica `EventKey::ActiveWindowChanged` só quando ela muda
//! (decisão pura em [`foreground::decide`]) — nunca no primeiro tick, mesma regra do TS
//! (`isFirstProbe` nunca emite).
//!
//! **Windows, Linux (X11) and macOS — wider than the TS.** `createActiveWindowWatcher().start()`
//! only armed the timer on win32; habits, the `technical` slang mode, the foreground game check and
//! the screen-capture foreground gate all read [`last_known`], so Linux/macOS users silently lost all
//! four. The per-OS probes live in `probe.rs` (Wayland is unsupported on purpose — see there);
//! without a probe the loop is never started and [`last_known`] stays `None`, the path the capture
//! gate already treats as fail-open.
//!
//! **Dependência nova: `windows` (crate oficial da Microsoft, dual MIT/Apache-2.0, sem RUSTSEC aberto
//! — checado em 2026-09-22 contra a advisory-db).** Ela JÁ está resolvida em `Cargo.lock` na versão
//! 0.62.2 — dependência transitiva de `sysinfo` e `xcap` — então declará-la como direta NÃO amplia a
//! árvore de dependências. O mesmo vale para `x11rb` (via `enigo`/`global-hotkey`) e `objc2-app-kit`
//! (via `xcap`), declaradas para Linux/macOS. `sysinfo` resolve o nome do processo dono a partir do PID.
//!
//! **Gate de config, DIVERGENTE do TS desde a varredura de controles de UI de 2026-09-27.** TS só
//! arma o watcher quando `brain.enabled` está ligado (`services/brain/index.ts`, ao lado de
//! `AwarenessService`/`StartupService`) — checado uma vez na ativação do plugin, não reativo a
//! mudança em runtime. Aqui o laço sempre sobe (plataforma permitindo) e [`spawn_active_window_watcher`]
//! lê `brain.enabled` A CADA tick: a Hero da aba "Memória" (`brain_tab.rs::declaration`,
//! `enabledConfigPath`) prometia um toggle mestre, e ligar/desligar em runtime não tinha nenhum
//! efeito sem reiniciar o processo — a fidelidade ao TS estava reproduzindo o MESMO defeito que a
//! varredura foi feita para achar, não uma escolha de porte. A blocklist de captura
//! (`vision.captureBlocklist`) NÃO se aplica a este watcher — verificado no TS: só
//! `capture-blocklist.ts`/`isBlockedForeground` a consultam, e nenhum chamador é o watcher de
//! janela. `redact.rs` cobre o que de fato se aplica: o título pode carregar PII (email, telefone) e
//! é redigido antes de publicar/guardar (`foreground::to_probe`), mesma garantia que
//! `foreground-window.ts::remember()` dava.
//!
//! **Estado compartilhado, não um assinante.** [`last_known`] guarda o último snapshot para leitura
//! PULL-BASED por `features::character::slang_context` (modo `technical`) — mesmo padrão de
//! `gamer::session_memory()` (`OnceLock<Mutex<Option<_>>>` de processo, não um `static` cru testado
//! direto: os testes deste módulo (`foreground.rs`) cobrem só as funções puras de decisão, nunca o
//! estado compartilhado).

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};

use super::foreground::{self, Decision};

const POLL_INTERVAL: Duration = Duration::from_millis(4_000);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Janela vista pela última sonda bem-sucedida — já com título/dono redigidos e `.exe` removido do
/// nome do processo. Consumida por `slang_context` via [`last_known`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveWindowSnapshot {
    pub title: Option<String>,
    pub owner: Option<String>,
}

impl ActiveWindowSnapshot {
    /// `isLikelyGame` de `getForegroundAppContext` — a mesma heurística do watcher.
    pub fn is_likely_game(&self) -> bool {
        foreground::is_likely_game_app(self.title.as_deref().unwrap_or(""), self.owner.as_deref().unwrap_or(""))
    }
}

fn snapshot_slot() -> &'static Mutex<Option<ActiveWindowSnapshot>> {
    static SLOT: OnceLock<Mutex<Option<ActiveWindowSnapshot>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Última janela ativa conhecida, ou `None` antes do primeiro tick real (ou em qualquer plataforma
/// sem sonda — Wayland, ver `probe.rs`).
pub fn last_known() -> Option<ActiveWindowSnapshot> {
    snapshot_slot().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `true` where a foreground probe exists (Windows, Linux on X11, macOS) — see `probe.rs`.
pub fn is_supported() -> bool {
    super::probe::unsupported_here().is_none()
}

pub(crate) fn remember(snapshot: ActiveWindowSnapshot) {
    *snapshot_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot);
}

/// `brain.enabled` lido do `ConfigStore` REAL — extraído de [`spawn_active_window_watcher`] para
/// ser testável sem subir o laço async inteiro (que sonda o SO de verdade). Default `true`
/// (config ausente/ilegível não desliga o watcher), mesma técnica de
/// `person_catalog::tool::catalog_enabled`.
fn brain_enabled(store: &ConfigStore) -> bool {
    store.get("brain.enabled").and_then(|v| v.as_bool()).unwrap_or(true)
}

/// Sobe o laço. `tauri::async_runtime::spawn`, nunca `tokio::spawn` — chamado de
/// `modules::register_config_bound_tools`, fora do reator (mesma regra do resto do boot). A sonda do
/// SO é bloqueante (Win32/X11/AppKit + `sysinfo`), por isso roda em `spawn_blocking` a cada tick,
/// mesma técnica de `system_signals::power::spawn_power_signal_listener`.
///
/// **`brain.enabled` é lido A CADA tick, não só aqui no boot** (varredura de controles de UI de
/// 2026-09-27): antes, um `false` no boot nunca subia o laço e um `true` depois na Settings nunca
/// era notado — a única forma de o toggle fazer efeito era reiniciar o processo inteiro. O corte de
/// plataforma continua estático (não muda em runtime, ao contrário do config).
/// A primeira leitura só "lembra" a identidade (não emite evento), mas o snapshot TEM de ser guardado
/// também: sem ele `last_known()` ficava vazio até a primeira troca de janela, e o gate de captura
/// (`vision::capture_gate::is_foreground_blocked`, fail-open sem snapshot) deixava capturar um
/// gerenciador de senhas/banco que já estava em foco quando a Mika abriu.
fn snapshot_to_keep(decision: &Decision, probe: &foreground::Probe) -> Option<ActiveWindowSnapshot> {
    matches!(decision, Decision::Remember(_) | Decision::Emit(_))
        .then(|| ActiveWindowSnapshot { title: probe.title.clone(), owner: probe.owner.clone() })
}

pub fn spawn_active_window_watcher(bus: &'static EventBus, store: Arc<ConfigStore>) {
    if let Some(reason) = super::probe::unsupported_here() {
        log::info!("watcher de janela ativa não iniciado: {reason}");
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut last_identity: Option<String> = None;
        let mut shutdown_rx = crate::shutdown::subscribe();

        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(crate::modules::system::governor::background(POLL_INTERVAL)) => {}
            }
            if !brain_enabled(&store) {
                continue;
            }
            let Ok(Some(raw)) = tokio::task::spawn_blocking(super::probe::probe_foreground).await else { continue };
            let probe = foreground::to_probe(&raw);

            let decision = foreground::decide(last_identity.as_deref(), &probe);
            if let Some(snapshot) = snapshot_to_keep(&decision, &probe) {
                remember(snapshot);
            }
            match decision {
                Decision::Skip | Decision::Unchanged => {}
                Decision::Remember(id) => last_identity = Some(id),
                Decision::Emit(id) => {
                    last_identity = Some(id);
                    bus.publish(
                        EventKey::ActiveWindowChanged,
                        json!({
                            "title": probe.title,
                            "owner": probe.owner,
                            "pid": probe.pid,
                            "isLikelyGame": probe.is_likely_game,
                            "at": now_ms(),
                        }),
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;

    fn store(enabled: bool) -> (ConfigStore, std::path::PathBuf) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-active-window-brain-enabled-{}-{unique}", std::process::id()));
        let schema = SchemaRegistry::new(vec![Namespace { name: "brain", fields: vec![boolean("enabled", enabled)] }]);
        (ConfigStore::open(&dir, schema).unwrap(), dir)
    }

    #[test]
    fn the_first_reading_is_kept_as_the_snapshot_even_though_it_emits_nothing() {
        let probe = foreground::Probe { title: Some("Bitwarden".into()), owner: Some("bitwarden".into()), pid: 7, is_likely_game: false };
        let first = foreground::decide(None, &probe);
        assert!(matches!(first, Decision::Remember(_)));
        assert_eq!(
            snapshot_to_keep(&first, &probe),
            Some(ActiveWindowSnapshot { title: Some("Bitwarden".into()), owner: Some("bitwarden".into()) })
        );
        let same = foreground::decide(Some("bitwarden|Bitwarden"), &probe);
        assert_eq!(snapshot_to_keep(&same, &probe), None);
    }

    #[test]
    fn brain_enabled_defaults_to_true_when_config_is_absent() {
        let dir = std::env::temp_dir().join(format!("mika-active-window-brain-enabled-absent-{}", std::process::id()));
        let store = ConfigStore::open(&dir, SchemaRegistry::new(vec![])).unwrap();
        assert!(brain_enabled(&store));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A leitura em si, agora extraída — a asserção que prova o gate: [`spawn_active_window_watcher`]
    /// só chama isto DENTRO do laço, a cada tick (ver o cabeçalho da função), então este teste é o
    /// que garante que `false` de verdade bloqueia, sem precisar subir o laço async inteiro contra
    /// o SO real.
    #[test]
    fn brain_enabled_reads_the_configured_value_live() {
        let (store, dir) = store(true);
        assert!(brain_enabled(&store));
        store.set("brain.enabled", serde_json::json!(false)).unwrap();
        assert!(!brain_enabled(&store));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
