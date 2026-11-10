//! Feature `mood` em Rust — port de `packages/mood/src` (motor puro) e do plugin
//! `apps/desktop/src/features/mood/index.ts` que o instancia e registra a tool `mood`.
//!
//! Diretório, não arquivo único, só pela regra de 300 linhas do AGENTS.md: [`math`] guarda as
//! funções puras (com os testes de paridade contra o TS), [`engine`] dá estado de processo a elas,
//! [`tool`] é a tool `mood` que o LLM chama (com os testes de despacho). A feature continua sendo
//! UMA unidade — `register_tools` (o único item exposto a `features::modules()`) é a costura entre
//! as três.
//!
//! ## O que porta de verdade
//! - `engine::MoodEngine` — port de `MoodService` (`services/mood.service.ts`): valência/
//!   excitação, rótulo por limiar.
//! - `engine::AffinityInner` — port de `AffinityService` (`services/affinity.service.ts`):
//!   pontuação, nível, rótulo por faixa.
//! - A tool `mood` (`tools/mood.tool.ts`), registrada de verdade no `ToolRegistry` nativo
//!   (`crate::modules::tools`) via [`register_tools`] — mesmas quatro ações
//!   (`get`/`update`/`register_interaction`/`decay`).
//! - [`needs`] — o corpo simulado (sono/sede/fome/tédio), a tool `needs` e o batimento que o
//!   persiste; lido pelo prompt (`brain::persona::routine_context`) e pelo motor proativo.
//!
//! ## Divergências do TS, documentadas, não mascaradas
//!
//! 1. **Estado de processo, não por ativação.** O TS cria uma instância nova a cada
//!    `activate()`/`deactivate()` do plugin. Aqui não existe esse ciclo de vida ainda (quem chama
//!    `features::modules()` — `ShellState::new()`, ver `state.rs` — roda uma vez, no boot), então
//!    o motor vive num `OnceLock` de processo, como `crate::modules::tools::registry()` já faz.
//!    Sem efeito observável hoje: nada reativa o app sem reiniciar o processo inteiro.
//! 2. **RESOLVIDO em 2026-09-21 (rodada de `register_config_bound_tools`).** Afinidade agora
//!    PERSISTE em `character.affinityScore` — ver [`persistence`]. `mood:changed`/
//!    `mood:affinity-level-up` são publicados (`tool.rs`/`persistence.rs`) via
//!    `EventKey::MoodChanged`/`MoodAffinityLevelUp`.
//! 3. **RESOLVIDO nesta rodada — `config/schemas/mood.rs` agora existe.** `mood.energyDecayRate`/
//!    `mood.affinityGainRate`/`mood.minAffinityInteractionTime` são configuráveis de verdade
//!    (aba `mood` em `crate::feature_registry::shared()`, ver [`persistence::register`]).
//!    `PersistentAffinity` lê `mood.affinityGainRate` FRESCO a cada `register_interaction` (mesmo
//!    padrão da leitura de `character.affinityScore`); [`boot::spawn_decay_timer`] lê
//!    `mood.energyDecayRate` a cada tick. `minAffinityInteractionTime` é o throttle de
//!    [`turn_mood`] — o listener de `USER_MESSAGE` do TS (`index.ts::activate`) que o porte perdeu.
//! 4. **RESOLVIDO nesta rodada — o timer de decaimento roda.** [`boot::spawn_decay_timer`],
//!    chamado por [`persistence::register`] (mesmo ponto que já tem o `ConfigStore`), reproduz o
//!    `setInterval(..., TICK_MS)` de `createMood` (`mood.runtime.ts`): a cada 60s, decai
//!    `valence`/`arousal` pelo tempo real passado e publica `EventKey::MoodChanged` só quando o
//!    RÓTULO muda sozinho — mesma condição de `onLabelChange`
//!    (`apps/desktop/src/features/mood/index.ts::activate`). `engine::tick` continua a mesma
//!    função testada; agora tem chamador.

mod affinity_notice;
mod boot;
mod engine;
mod math;
pub(crate) mod needs;
mod persistence;
mod plugin_test;
mod rpc;
mod tool;
mod turn_mood;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::store::ConfigStore;

pub(crate) use tool::register_tools;

/// `mood.enabled` (toggle mestre da aba) cacheado em processo — [`current_mood_label`] é chamada
/// por `chat::persona_composer::gather_persona_state` (fora deste módulo, sem `ConfigStore`/
/// `AppHandle` no ponto de chamada), então não pode ler o store fresco como o resto da feature já
/// faz. [`refresh_master_enabled`] é chamada pelos dois pontos que JÁ recebem `Arc<ConfigStore>`
/// ([`boot::spawn_decay_timer`] a cada tick de 60s, [`turn_mood::spawn_turn_mood_listener`] a cada
/// turno real) — o atraso máximo entre desligar a aba e o prompt parar de carregar humor é o
/// menor desses dois intervalos, nunca "nunca" (varredura de controles de UI de 2026-09-27: o
/// toggle mestre não gateava decaimento, afinidade, NEM o bloco de humor no prompt).
static MASTER_ENABLED: AtomicBool = AtomicBool::new(true);

/// Lê `mood.enabled` do `ConfigStore` REAL e atualiza o cache de processo que
/// [`current_mood_label`] consulta. Devolve o mesmo valor lido, para o chamador decidir se
/// aplica o efeito de decaimento/interação/emoção também — default `true` (config
/// ausente/ilegível não desliga a feature), mesma técnica de
/// `person_catalog::tool::catalog_enabled`.
pub(crate) fn refresh_master_enabled(store: &ConfigStore) -> bool {
    let enabled = store.get("mood.enabled").and_then(|v| v.as_bool()) != Some(false);
    MASTER_ENABLED.store(enabled, Ordering::Relaxed);
    enabled
}
/// Segunda passada de registro — ver [`persistence`] e `modules::register_config_bound_tools`
/// (`modules/mod.rs`, fora do meu escopo, é quem chama isto de `lib.rs::setup`). Também dispara
/// [`boot::spawn_decay_timer`] — ver o cabeçalho de `boot.rs`.
pub use affinity_notice::spawn_affinity_notice_listener;
pub use persistence::register;

/// Rótulo (pt-BR) do humor ATUAL — acessor mínimo para `chat::persona_composer::gather_persona_state`
/// (fora deste módulo). Divergência documentada do TS: `brainStatePort().getEmotionalState().mood`
/// (o estado emocional do `brain`, fase 7) não existe em Rust; este motor de valência/excitação
/// (`engine::mood_snapshot`) é a fonte funcional mais próxima disponível, não o mesmo dado — ver o
/// cabeçalho de `chat/persona_composer.rs` para a decisão. Mapeia os 7 rótulos em inglês do motor
/// (`math::label_for`) para os 7 nomes em português que `persona_composer::mood_frame` já reconhece.
pub(crate) fn current_mood_label() -> String {
    if !MASTER_ENABLED.load(Ordering::Relaxed) {
        return "neutra".to_string();
    }
    let snapshot = engine::engine().mood_snapshot();
    let label = snapshot["label"].as_str().unwrap_or("neutral");
    match label {
        "excited" => "animada",
        "content" => "satisfeita",
        "happy" => "feliz",
        "tense" => "irritada",
        "bored" => "entediada",
        "sad" => "triste",
        _ => "neutra",
    }
    .to_string()
}

/// Serializa os testes que registram a tool `mood` no catálogo de processo.
///
/// `tools::registry()` é um `OnceLock` compartilhado e o `cargo test` roda em paralelo: um teste que
/// registra a versão SEM config e outro que registra a versão COM config disputam a mesma chave, e
/// quem escreve por último decide o resultado dos dois. A falha aparece como flake — passa isolado,
/// quebra na suíte cheia — e some se alguém "corrigir" a asserção, que é o pior desfecho, porque a
/// asserção está certa: `register` TEM de sobrescrever `register_tools`.
///
/// Um mutex é a resposta honesta. Ele não enfraquece nada: só impede que dois testes escrevam na
/// mesma chave global ao mesmo tempo. Mesma razão pela qual asserção negativa contra singleton não é
/// prova — ver o cabeçalho de `events/bus.rs`.
#[cfg(test)]
pub(crate) static TOOL_REGISTRATION_ORDER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `MASTER_ENABLED` é `static` de processo e `cargo test` roda em paralelo — qualquer teste (deste
/// arquivo ou de [`boot`]/[`turn_mood`]) que grava nele precisa desta trava primeiro, mesma classe
/// de bug que [`TOOL_REGISTRATION_ORDER`] guarda.
#[cfg(test)]
pub(crate) static MASTER_ENABLED_TEST_ORDER: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mood-mod-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn config_store(enabled: bool) -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace { name: "mood", fields: vec![boolean("enabled", enabled)] }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    /// Toggle mestre (`mood.enabled=false`) tem de zerar o bloco de humor que
    /// `chat::persona_composer::gather_persona_state` monta no prompt — antes desta rodada
    /// `current_mood_label()` nunca lia o toggle, só o motor de valência/excitação.
    #[test]
    fn current_mood_label_falls_back_to_neutral_when_the_master_toggle_is_off() {
        let _guard = MASTER_ENABLED_TEST_ORDER.lock().unwrap();
        let (enabled_store, _dir) = config_store(true);
        assert!(refresh_master_enabled(&enabled_store));

        let (disabled_store, _dir2) = config_store(false);
        assert!(!refresh_master_enabled(&disabled_store));
        assert_eq!(current_mood_label(), "neutra");

        // Restaura o default para não vazar estado para outros testes deste binário.
        refresh_master_enabled(&enabled_store);
    }

    #[test]
    fn refresh_master_enabled_defaults_to_true_when_config_is_absent() {
        let _guard = MASTER_ENABLED_TEST_ORDER.lock().unwrap();
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, SchemaRegistry::new(vec![])).unwrap();
        assert!(refresh_master_enabled(&store));
    }
}
