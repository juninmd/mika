//! Energia: tempo de ocioso, bateria e bloqueadores de sono. Moram aqui porque só o SO sabe
//! deles.
//!
//! **Lacuna nomeada — o poller de energia ficou sem consumidor em 2026-09-22, revivida em parte na
//! mesma data.** Ele empurrava `power-suspend`/`power-resume`/`power-idle-tick`/`power-on-battery`
//! ao sidecar, que os traduzia para o `powerMonitor` do Electron e daí para
//! `desktop-main.module.ts` e `prompt-compose.module.ts`. Com o sidecar apagado, todos os
//! consumidores tinham deixado de existir. `modules/system_signals/power.rs` agora publica
//! `EventKey::PowerResume`/`PowerBattery` (as duas reações reais que `awareness-event-handlers.ts`
//! ainda tinha: "sistema acordou"/"saiu da tomada" — ver `brain::awareness::event_reactions`) por
//! polling de 15s, detectando resume pelo salto no relógio de parede entre dois ticks e battery
//! pela borda de transição de [`on_battery_power`]. `power-idle-tick` e `power-suspend` continuam
//! SEM consumidor Rust — nenhuma reação de `awareness-event-handlers.ts` os usava (só
//! `proactive-engine.ts::onSuspend` cancelava o timer local, não uma fala), então não entraram em
//! `events/keys.rs` (regra: consumidor real antes da chave).
//!
//! O que NÃO sumiu: [`snapshot`] é síncrono e tem chamador real (`modules/system`, a ação de
//! shell `getPowerSnapshot`, e desde 2026-09-22 também
//! `modules/brain/awareness/providers.rs::IdleProvider`). "Quanto tempo de ocioso agora?" continua
//! respondido pontualmente; `IdleProvider` funciona por polling próprio (a cada
//! `PROACTIVE_BASE_INTERVAL_MS`), não pelo empurrão periódico deste arquivo.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};
use starship_battery::State as BatteryState;

struct PowerRuntime {
    blockers: Mutex<HashMap<u64, keepawake::KeepAwake>>,
    next_blocker_id: AtomicU64,
}

static RUNTIME: OnceLock<PowerRuntime> = OnceLock::new();

fn runtime() -> &'static PowerRuntime {
    RUNTIME.get_or_init(|| PowerRuntime {
        blockers: Mutex::new(HashMap::new()),
        next_blocker_id: AtomicU64::new(1),
    })
}

/// Segundos desde o último input do usuário. `GetLastInputInfo` no Windows,
/// `CGEventSourceSecondsSinceLastEventType`-equivalente (IOKit HIDIdleTime) no macOS,
/// extensão XScreenSaver no Linux — a crate `user-idle` encapsula os três.
pub fn idle_seconds() -> Result<u64, String> {
    // A `user-idle` 0.6 chama `XOpenDisplay` e usa o ponteiro sem checar NULL: sem `DISPLAY`
    // (Wayland puro, sessão headless, CI) o processo inteiro morria de SIGSEGV. Vira erro nomeado.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
        return Err("sem servidor X (DISPLAY vazio): tempo ocioso indisponível".into());
    }
    user_idle::UserIdle::get_time()
        .map(|idle| idle.as_seconds())
        .map_err(|e| e.to_string())
}

/// `true` = rodando na bateria. Sem bateria detectada é FATO (desktop sem bateria está sempre
/// na tomada), não chute — só um erro real de enumeração vira `Err` (ver AGENTS.md: nunca
/// devolver um valor plausível sem medir). `pub` desde `modules/system_signals/power.rs`: o
/// poller de energia lê a borda de transição em vez de reimplementar a mesma enumeração.
pub fn on_battery_power() -> Result<bool, String> {
    let manager = starship_battery::Manager::new().map_err(|e| e.to_string())?;
    let batteries = manager.batteries().map_err(|e| e.to_string())?;

    for entry in batteries {
        match entry {
            Ok(battery) => {
                // `Paused` (limite de carga atingido) e `Full`/`Charging`/`Unknown` continuam
                // ligados na tomada; só `Discharging`/`Empty` são "na bateria" de verdade.
                return Ok(matches!(
                    battery.state(),
                    BatteryState::Discharging | BatteryState::Empty
                ));
            }
            Err(error) => log::warn!("bateria: entrada inválida ignorada: {error}"),
        }
    }

    Ok(false)
}

/// Leitura pontual de ocioso + bateria. Síncrona de propósito: é a única forma de responder
/// "agora" sem o empurrão periódico que perdeu os consumidores (ver o cabeçalho).
pub fn snapshot() -> Result<Value, String> {
    let idle_seconds = idle_seconds()?;
    let on_battery_power = on_battery_power()?;

    Ok(json!({
        "idleSeconds": idle_seconds,
        "onBatteryPower": on_battery_power,
    }))
}


/// `powerSaveBlocker.start(type)`. `prevent-display-sleep` também mantém a tela acesa;
/// qualquer outro tipo (hoje só `prevent-app-suspension`, ver `system-metrics.ts`) impede só
/// o sono por ociosidade do sistema.
pub fn start_blocker(kind: &str) -> Result<u64, String> {
    let keep_display = kind == "prevent-display-sleep";

    let awake = keepawake::Builder::default()
        .display(keep_display)
        .idle(true)
        .reason("Mika ativa")
        .app_name("Mika")
        .app_reverse_domain("com.mika.desktop")
        .create()
        .map_err(|e| e.to_string())?;

    let rt = runtime();
    let id = rt.next_blocker_id.fetch_add(1, Ordering::Relaxed);
    rt.blockers
        .lock()
        .map_err(|_| "lock de bloqueadores de energia envenenado".to_string())?
        .insert(id, awake);

    Ok(id)
}

/// `powerSaveBlocker.stop(id)`. Soltar um id desconhecido (já parado, ou nunca existiu) é
/// no-op silencioso — mesmo contrato do Electron, que não lança para id inválido.
pub fn stop_blocker(id: u64) -> Result<(), String> {
    runtime()
        .blockers
        .lock()
        .map_err(|_| "lock de bloqueadores de energia envenenado".to_string())?
        .remove(&id);
    Ok(())
}
