//! Timer de decaimento de humor — port do `setInterval(..., TICK_MS)` de `createMood`
//! (`packages/mood/src/services/mood.runtime.ts`). Fecha o item 3 do cabeçalho de `mod.rs`
//! (`tick()` portado, testado, sem chamador).
//!
//! [`spawn_decay_timer`] é chamado por [`super::persistence::register`] — o mesmo ponto que já
//! recebe o `ConfigStore` de verdade (`modules::register_config_bound_tools`, fora do meu escopo).
//! Nenhuma outra ligação é necessária: o segundo registro da tool `mood` e o boot do timer
//! acontecem juntos, na mesma chamada.
//!
//! **`tokio::spawn`/`tauri::async_runtime::spawn` só AGENDA a task — não roda nada até o
//! executor ceder o turno.** Isso importa quando a task ASSINA o barramento (`bus().subscribe()`
//! DEPOIS do spawn perde qualquer evento publicado antes da primeira iteração, porque
//! `broadcast::Sender` não entrega retroativamente — o mesmo bug que já custou uma rodada no
//! listener de TTS, `modules/voice/tts/bus_bridge.rs`). Esta task não se aplica: ela só PUBLICA
//! (`bus().publish`), nunca assina nada, então não há janela de corrida para perder eventos.

use std::sync::Arc;
use std::time::Duration;

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

use super::engine::engine;
use super::math::DEFAULT_DECAY_RATE;

/// Espelha `TICK_MS`/`TICK_SECONDS` de `mood.runtime.ts`.
const TICK_INTERVAL: Duration = Duration::from_secs(60);
const TICK_SECONDS: f64 = 60.0;
const DECAY_RATE_CONFIG_PATH: &str = "mood.energyDecayRate";

/// Lido FRESCO a cada tick — mesmo padrão de `persistence::PersistentAffinity`: a taxa é
/// configurável em runtime (`config/schemas/mood.rs`), então um valor cacheado no boot ficaria
/// desatualizado se o usuário mudar o slider depois.
fn effective_decay_rate(store: &ConfigStore) -> f64 {
    store.get(DECAY_RATE_CONFIG_PATH).and_then(|v| v.as_f64()).unwrap_or(DEFAULT_DECAY_RATE)
}

/// Um tick isolado — puro o bastante pra testar sem esperar 60s de verdade. Devolve o snapshot só
/// quando o RÓTULO muda, nunca a cada chamada: mesma condição de `onLabelChange`
/// (`apps/desktop/src/features/mood/index.ts::activate`), que só emite `MOOD_CHANGED` quando o
/// decaimento sozinho empurra o humor para outro rótulo — não a cada tick silencioso.
fn tick_once(store: &ConfigStore) -> Option<serde_json::Value> {
    // Toggle mestre (`mood.enabled`) — desligado, o decaimento não roda e a leitura fresca
    // realimenta o cache de processo que `super::current_mood_label` consulta (ver o cabeçalho de
    // `super::refresh_master_enabled`; varredura de controles de UI de 2026-09-27).
    if !super::refresh_master_enabled(store) {
        return None;
    }
    let previous_label = engine().mood_snapshot()["label"].as_str().unwrap_or_default().to_string();
    let snapshot = engine().tick(effective_decay_rate(store), TICK_SECONDS);
    if snapshot["label"].as_str() == Some(previous_label.as_str()) {
        return None;
    }
    Some(snapshot)
}

/// Chamado UMA VEZ no boot. Ver o cabeçalho do arquivo para por que assinar o barramento não é um
/// risco aqui (esta task nunca assina, só publica).
pub fn spawn_decay_timer(store: Arc<ConfigStore>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        ticker.tick().await; // o primeiro tick de `interval` é imediato; descarta pra não decair no instante 0.
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = ticker.tick() => {}
            }
            if let Some(snapshot) = tick_once(&store) {
                bus().publish(EventKey::MoodChanged, snapshot);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{boolean, float_range};
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mood-boot-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "mood",
            fields: vec![boolean("enabled", true), float_range("energyDecayRate", 0.02, 0.0, 1.0)],
        }]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    /// Toggle mestre (`mood.enabled=false`) tem de parar o decaimento — antes desta rodada
    /// `tick_once` só existia gated por nada, o timer sempre decaía e publicava
    /// `EventKey::MoodChanged` mesmo com a aba desligada.
    #[test]
    fn tick_once_is_a_no_op_when_the_master_toggle_is_off() {
        // `MASTER_ENABLED` (`super::MASTER_ENABLED_TEST_ORDER`) é `static` de processo — mesma
        // trava que `mood::mod::tests` usa, porque este teste também escreve nela via
        // `refresh_master_enabled` (dentro de `tick_once`).
        let _guard = super::super::MASTER_ENABLED_TEST_ORDER.lock().unwrap();
        let (config, _dir) = store();
        config.set("mood.enabled", json!(false)).unwrap();
        // `engine()` é `OnceLock` de processo, compartilhado com outros arquivos de teste do
        // mesmo `cargo test` em paralelo — o gate tem de curto-circuitar ANTES de tocar nele, então
        // não há necessidade (nem é seguro) de mutar o estado global aqui para provar isto.
        assert_eq!(tick_once(&config), None, "mood.enabled=false não pode decair nem trocar de rótulo");
    }

    #[test]
    fn effective_decay_rate_reads_the_configured_value() {
        let (config, _dir) = store();
        assert_eq!(effective_decay_rate(&config), 0.02, "default do schema");

        config.set("mood.energyDecayRate", json!(0.5)).unwrap();
        assert_eq!(effective_decay_rate(&config), 0.5, "não leu o valor configurado");
    }

    /// NEGATIVO — namespace `mood` ausente (schema de teste sem ele) cai no default embutido,
    /// nunca em pânico.
    #[test]
    fn effective_decay_rate_falls_back_when_the_namespace_is_absent() {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![]);
        let config = ConfigStore::open(&dir.0, schema).unwrap();
        assert_eq!(effective_decay_rate(&config), DEFAULT_DECAY_RATE);
    }
}
