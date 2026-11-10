//! Governador de energia — o ÚNICO lugar que decide quanto a Mika pode gastar agora. A decisão é
//! pura ([`policy`]); este arquivo guarda os sinais (atômicos de processo, lidos por todo laço de
//! fundo sem lock), assina o barramento para jogo/bateria/conversa, e avisa o renderer
//! (`power:profile`) quando o orçamento muda — o teto de fps do avatar vem daí.
//!
//! **Estado de processo, não `Arc` injetado.** Quem lê são laços espalhados por oito módulos
//! (visão, clipboard, cursor, janela ativa, memória, awareness, ONNX); injetar um `Arc` em cada um
//! mudaria a assinatura de todos os `spawn_*` para ler três atômicos. Mesmo corte de
//! `system_signals::active_window::last_known`.
//!
//! Sinais: jogo por `GameChanged`/`GameStopped`; bateria e ócio do SO pelo poller de energia
//! (`system_signals/power.rs`, [`set_power_readings`]); conversa por `ThinkingStart` e
//! `ActivityChanged{listening|thinking}`; perfil por `app.performanceProfile` (observador do
//! `ConfigStore`, efeito imediato). O payload também leva a aceleração dos modelos ONNX
//! (`inference::accel`): a preferência `app.inferenceAcceleration` é lida aqui, junto do perfil, e
//! cada carga de modelo chama [`nudge`] para a leitura ao vivo não esperar o próximo tick.

pub mod policy;

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};
use policy::{Budget, PowerState, Profile, Signals};

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.PowerProfile` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`).
pub const PROFILE_EVENT: &str = "power:profile";
pub const PROFILE_CONFIG_PATH: &str = "app.performanceProfile";
/// Reavaliação sem evento: a passagem para ocioso é por tempo, e o descarte de modelo ocioso também.
const REEVALUATE: Duration = Duration::from_secs(30);
const UNKNOWN_IDLE: u64 = u64::MAX;

struct Signal {
    profile: AtomicU8,
    gaming: AtomicBool,
    on_battery: AtomicBool,
    os_idle_s: AtomicU64,
    last_interaction_ms: AtomicU64,
}

static SIGNAL: Signal = Signal {
    profile: AtomicU8::new(0),
    gaming: AtomicBool::new(false),
    on_battery: AtomicBool::new(false),
    os_idle_s: AtomicU64::new(UNKNOWN_IDLE),
    last_interaction_ms: AtomicU64::new(0),
};

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn uptime_ms() -> u64 {
    epoch().elapsed().as_millis() as u64
}

const PROFILES: [Profile; 3] = [Profile::Auto, Profile::Economy, Profile::Quality];

pub fn profile() -> Profile {
    PROFILES[SIGNAL.profile.load(Ordering::Relaxed) as usize % PROFILES.len()]
}

fn set_profile(profile: Profile) {
    let index = PROFILES.iter().position(|p| *p == profile).unwrap_or(0);
    SIGNAL.profile.store(index as u8, Ordering::Relaxed);
}

/// Chamado pelo poller de energia a cada leitura (15s).
pub fn set_power_readings(on_battery: bool, os_idle: Option<Duration>) {
    SIGNAL.on_battery.store(on_battery, Ordering::Relaxed);
    SIGNAL.os_idle_s.store(os_idle.map_or(UNKNOWN_IDLE, |d| d.as_secs()), Ordering::Relaxed);
}

fn note_interaction() {
    SIGNAL.last_interaction_ms.store(uptime_ms(), Ordering::Relaxed);
}

fn signals() -> Signals {
    let idle = SIGNAL.os_idle_s.load(Ordering::Relaxed);
    Signals {
        gaming: SIGNAL.gaming.load(Ordering::Relaxed),
        on_battery: SIGNAL.on_battery.load(Ordering::Relaxed),
        os_idle: (idle != UNKNOWN_IDLE).then(|| Duration::from_secs(idle)),
        since_interaction: Duration::from_millis(uptime_ms().saturating_sub(SIGNAL.last_interaction_ms.load(Ordering::Relaxed))),
    }
}

pub fn state() -> PowerState {
    policy::state_for(&signals())
}

pub fn budget() -> Budget {
    policy::budget(profile(), state())
}

/// Cadência de um laço de manutenção: `base` escalado pelo orçamento atual.
pub fn background(base: Duration) -> Duration {
    policy::scaled(base, budget().background_scale)
}

/// Cadência da captura+análise de tela.
pub fn vision(base: Duration) -> Duration {
    policy::scaled(base, budget().vision_scale)
}

fn physical_cores() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| {
        sysinfo::System::physical_core_count()
            .or_else(|| std::thread::available_parallelism().ok().map(|n| (n.get() / 2).max(1)))
            .unwrap_or(2)
    })
}

/// Threads intra-op para uma sessão ONNX que vai ser criada AGORA (vale até ela ser descarregada).
pub fn onnx_threads() -> usize {
    policy::onnx_threads(physical_cores(), budget().halve_threads)
}

/// Resposta de `mika:get-power-state` e payload de [`PROFILE_EVENT`].
pub fn snapshot() -> Value {
    let (profile, state) = (profile(), state());
    let budget = policy::budget(profile, state);
    json!({
        "profile": profile.as_str(),
        "state": state.as_str(),
        "ceiling": budget.ceiling.as_str(),
        "backgroundScale": budget.background_scale,
        "visionScale": budget.vision_scale,
        "onnxThreads": policy::onnx_threads(physical_cores(), budget.halve_threads),
        "acceleration": crate::modules::inference::accel::snapshot(),
    })
}

fn nudged() -> &'static tokio::sync::Notify {
    static NUDGE: OnceLock<tokio::sync::Notify> = OnceLock::new();
    NUDGE.get_or_init(tokio::sync::Notify::new)
}

/// Reavalia e reemite agora — chamado quando algo fora dos sinais mudou o snapshot (uma sessão ONNX
/// carregou, e com ela o provedor efetivo).
pub fn nudge() {
    nudged().notify_one();
}

fn is_interaction(key: EventKey, payload: &Value) -> bool {
    match key {
        EventKey::ThinkingStart => true,
        EventKey::ActivityChanged => matches!(payload.get("state").and_then(Value::as_str), Some("listening" | "thinking")),
        _ => false,
    }
}

/// `true` quando o evento mexeu num sinal — só aí vale recalcular o orçamento.
fn apply_event(key: EventKey, payload: &Value) -> bool {
    match key {
        EventKey::GameChanged => SIGNAL.gaming.store(true, Ordering::Relaxed),
        EventKey::GameStopped => SIGNAL.gaming.store(false, Ordering::Relaxed),
        EventKey::PowerBattery => SIGNAL.on_battery.store(true, Ordering::Relaxed),
        EventKey::PowerAc => SIGNAL.on_battery.store(false, Ordering::Relaxed),
        _ if is_interaction(key, payload) => note_interaction(),
        _ => return false,
    }
    true
}

fn emit(app: &AppHandle, payload: &Value) {
    for label in crate::protocol::renderer_event_targets(app.webview_windows().keys().cloned().collect()) {
        let _ = app.emit_to(label.as_str(), PROFILE_EVENT, payload.clone());
    }
}

/// Sobe o governador: lê o perfil, observa a config, assina o barramento e reavalia a cada evento
/// ou [`REEVALUATE`]. Cada reavaliação também descarrega modelo ONNX ocioso.
pub fn spawn(bus: &'static EventBus, store: Arc<ConfigStore>, app: AppHandle) {
    note_interaction();
    use crate::modules::inference::accel::{self, Preference};
    let text = |value: Option<&Value>| value.and_then(Value::as_str).unwrap_or("auto").to_string();
    set_profile(Profile::parse(&text(store.get(PROFILE_CONFIG_PATH).as_ref())));
    accel::set_preference(Preference::parse(&text(store.get(accel::CONFIG_PATH).as_ref())));
    let changed = Arc::new(tokio::sync::Notify::new());
    let notify = Arc::clone(&changed);
    store.watch(Box::new(move |path, value| {
        match path {
            PROFILE_CONFIG_PATH => set_profile(Profile::parse(&text(Some(value)))),
            accel::CONFIG_PATH => accel::set_preference(Preference::parse(&text(Some(value)))),
            _ => return,
        }
        notify.notify_one();
    }));
    let mut sub = bus.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        let mut reevaluate = tokio::time::interval(REEVALUATE);
        reevaluate.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last = Value::Null;
        loop {
            let relevant = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = changed.notified() => true,
                _ = nudged().notified() => true,
                _ = reevaluate.tick() => {
                    if let Some(max_idle) = budget().model_idle_unload {
                        tauri::async_runtime::spawn_blocking(move || crate::modules::inference::evict_idle(max_idle));
                    }
                    true
                }
                received = sub.recv() => match received {
                    Some(envelope) => apply_event(envelope.key, &envelope.payload),
                    None => break,
                },
            };
            if !relevant {
                continue;
            }
            let snapshot = snapshot();
            if snapshot != last {
                log::info!("governador de energia: {snapshot}");
                emit(&app, &snapshot);
                last = snapshot;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_literal_exists_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("'{PROFILE_EVENT}'")));
    }

    /// Só a conversa conta como interação — a Mika falando sozinha (proativa) não mantém o PC "ativo".
    #[test]
    fn only_listening_or_a_user_turn_counts_as_interaction() {
        assert!(is_interaction(EventKey::ThinkingStart, &Value::Null));
        assert!(is_interaction(EventKey::ActivityChanged, &json!({ "state": "listening" })));
        assert!(!is_interaction(EventKey::ActivityChanged, &json!({ "state": "speaking" })));
        assert!(!is_interaction(EventKey::GameChanged, &Value::Null));
    }
}
