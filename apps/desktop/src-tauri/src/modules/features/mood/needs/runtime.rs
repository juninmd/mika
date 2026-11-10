//! Estado de processo do corpo: o [`BodyClock`] persistido em `<dados>/body-state.json`, o
//! batimento (boot + a cada [`HEARTBEAT`]) que detecta sono e pede o bocejo ao avatar, e o ouvinte
//! de turno que zera o tédio quando o usuário fala com ela.
//!
//! Config lida FRESCA a cada leitura (`mood.enabled`, `mood.needsEnabled`, `mood.needsIntensity`):
//! desligar na Settings tira o corpo do prompt no turno seguinte, sem reiniciar.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::Timelike;
use serde_json::json;

use crate::config::atomic::write_atomic;
use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

use super::{needs_at, BodyClock, Need, Needs, STRONG};

const HEARTBEAT: Duration = Duration::from_secs(5 * 60);
/// Um bocejo a cada tanto — a cada batimento viraria tique.
const HINT_COOLDOWN_MS: u64 = 20 * 60 * 1000;
const FILE_NAME: &str = "body-state.json";

pub(crate) struct Body {
    path: PathBuf,
    clock: Mutex<BodyClock>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Body {
    /// Lê o relógio salvo (ou nasce agora) e já aplica o batimento de boot: app fechado a noite toda
    /// = ela dormiu.
    fn open(dir: &Path, now_ms: u64) -> Self {
        let path = dir.join(FILE_NAME);
        let saved = std::fs::read_to_string(&path).ok().and_then(|raw| serde_json::from_str::<BodyClock>(&raw).ok());
        let body = Self { path, clock: Mutex::new(saved.unwrap_or_else(|| BodyClock::fresh(now_ms))) };
        body.update(|clock| clock.observe(now_ms, 0).0);
        body
    }

    pub(crate) fn snapshot(&self) -> BodyClock {
        *self.clock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn update(&self, change: impl FnOnce(BodyClock) -> BodyClock) -> BodyClock {
        let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        *clock = change(*clock);
        if let Err(error) = write_atomic(&self.path, &serde_json::to_vec(&*clock).unwrap_or_default()) {
            log::warn!("needs: {} não gravado: {error}", self.path.display());
        }
        *clock
    }
}

static BODY: OnceLock<Body> = OnceLock::new();

pub(crate) fn body() -> Option<&'static Body> {
    BODY.get()
}

/// `mood.enabled` E `mood.needsEnabled` — o toggle mestre da aba desliga o corpo junto.
pub(crate) fn enabled(store: &ConfigStore) -> bool {
    let on = |path: &str| store.get(path).and_then(|v| v.as_bool()) != Some(false);
    on("mood.enabled") && on("mood.needsEnabled")
}

pub(crate) fn intensity(store: &ConfigStore) -> f64 {
    store.get("mood.needsIntensity").and_then(|v| v.as_f64()).unwrap_or(1.0)
}

fn user_hour(store: &ConfigStore) -> f64 {
    let local = crate::modules::brain::persona::user_clock::user_now(store).local;
    f64::from(local.hour()) + f64::from(local.minute()) / 60.0
}

/// As necessidades AGORA, ou `None` com o corpo desligado/ainda não montado.
pub(crate) fn current_needs(store: &ConfigStore) -> Option<Needs> {
    if !enabled(store) {
        return None;
    }
    let clock = body()?.snapshot();
    Some(needs_at(&clock, now_ms(), user_hour(store), intensity(store)))
}

/// Gesto do avatar para o estado do corpo — só sono forte vira bocejo/cochilo, com intervalo.
fn idle_hint(needs: &Needs, last_hint_ms: Option<u64>, now_ms: u64) -> Option<&'static str> {
    if needs.sleepiness < STRONG || last_hint_ms.is_some_and(|last| now_ms.saturating_sub(last) < HINT_COOLDOWN_MS) {
        return None;
    }
    Some(if needs.sleepiness >= 0.9 { "doze" } else { "yawn" })
}

/// Gesto de quando ela de fato satisfaz a necessidade (tool `needs`, botão da Settings).
pub(crate) fn satisfy_gesture(need: Need) -> Option<&'static str> {
    match need {
        Need::Drink => Some("drink"),
        Need::Sleep => Some("stretch"),
        Need::Eat => Some("relaxed"),
        Need::Play => None,
    }
}

/// Pelo tool `avatar` já existente — mesma reversão para `neutral` e o mesmo cancelamento por
/// geração que o LLM usa, nada paralelo.
pub(crate) async fn play_gesture(expression: &str) {
    let args = json!({ "action": "expression", "expression": expression, "durationMs": 4000 });
    if let Err(error) = crate::modules::tools::registry().execute("avatar", args).await {
        log::debug!("needs: gesto '{expression}' não tocou: {error}");
    }
}

async fn heartbeat(store: &ConfigStore, body: &Body, last_hint: &mut Option<u64>) {
    let idle_ms = tokio::task::spawn_blocking(crate::power::idle_seconds).await.ok().and_then(Result::ok).unwrap_or(0) * 1000;
    let now = now_ms();
    body.update(|clock| clock.observe(now, idle_ms).0);
    let Some(needs) = current_needs(store) else { return };
    if let Some(gesture) = idle_hint(&needs, *last_hint, now) {
        *last_hint = Some(now);
        play_gesture(gesture).await;
    }
}

/// Chamado uma vez por [`super::register`]. Assina o barramento ANTES do spawn (broadcast não
/// reentrega).
pub(super) fn spawn(store: Arc<ConfigStore>, dir: &Path) {
    let body = BODY.get_or_init(|| Body::open(dir, now_ms()));
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(HEARTBEAT);
        let mut shutdown_rx = crate::shutdown::subscribe();
        let mut last_hint = None;
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = ticker.tick() => heartbeat(&store, body, &mut last_hint).await,
                received = sub.recv() => match received {
                    Some(envelope) if envelope.key == EventKey::ThinkingStart
                        && !crate::modules::brain::post_turn::is_background_turn(&envelope.payload) => {
                        body.update(|clock| clock.talked(now_ms()));
                    }
                    Some(_) => {}
                    None => break,
                },
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::mood::needs::HOUR_MS;

    fn dir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mika-needs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    /// O corpo sobrevive ao reinício: o que ela bebeu antes de fechar continua valendo depois, e
    /// uma noite com o app fechado conta como sono.
    #[test]
    fn the_body_persists_across_restarts_and_a_closed_night_is_sleep() {
        let dir = dir("persist");
        let t0 = 500 * HOUR_MS;
        let body = Body::open(&dir, t0);
        body.update(|c| c.satisfy(Need::Drink, t0 + HOUR_MS));

        let reopened = Body::open(&dir, t0 + 2 * HOUR_MS);
        assert_eq!(reopened.snapshot().drank_at_ms, t0 + HOUR_MS);
        assert_eq!(reopened.snapshot().woke_at_ms, t0, "2h fechado não é sono");

        let next_morning = Body::open(&dir, t0 + 14 * HOUR_MS);
        assert_eq!(next_morning.snapshot().woke_at_ms, t0 + 14 * HOUR_MS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_file_starts_a_fresh_body_instead_of_failing() {
        let dir = dir("corrupt");
        std::fs::write(dir.join(FILE_NAME), "{nope").unwrap();
        assert_eq!(Body::open(&dir, 42).snapshot(), BodyClock::fresh(42));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_strong_sleepiness_yawns_and_not_every_heartbeat() {
        let sleepy = Needs { sleepiness: 0.8, ..Needs::default() };
        assert_eq!(idle_hint(&sleepy, None, 1_000), Some("yawn"));
        assert_eq!(idle_hint(&Needs { sleepiness: 0.95, ..sleepy }, None, 1_000), Some("doze"));
        assert_eq!(idle_hint(&sleepy, Some(1_000), 1_000 + HINT_COOLDOWN_MS - 1), None);
        assert_eq!(idle_hint(&Needs { sleepiness: 0.6, ..sleepy }, None, 1_000), None);
    }

    /// Os gestos pedidos ao avatar existem no catálogo da tool `avatar` — um nome fora dele é
    /// recusado pela validação e o gesto some sem erro visível.
    #[test]
    fn every_gesture_is_an_avatar_expression() {
        let catalog = crate::modules::features::character::model_inventory::EXPRESSIONS;
        let gestures = ["yawn", "doze"].into_iter().chain([Need::Sleep, Need::Drink, Need::Eat, Need::Play].into_iter().filter_map(satisfy_gesture));
        for gesture in gestures {
            assert!(catalog.contains(&gesture), "{gesture}");
        }
    }
}
