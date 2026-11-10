//! === persona/persona-composer.ts + chat/turn-context-sources.ts::gatherPersonaState ============
//!
//! [`inject_persona_state`] espelha `PersonaComposer.injectPersonaState`, que
//! `turn-context.ts::composeSystemPrompt` aplica DEPOIS de `buildSystemPrompt`: humor e vínculo
//! entram logo antes de "## CRITICAL STYLE RULES" (que vem de `system-tail.md`). Só o turno de chat
//! injeta (`brain::persona::compose_system_prompt`); o caminho "raw" (`generateRaw`/proativo) não,
//! igual ao TS.
//!
//! `PersonaComposer.buildSystemPrompt` (o composer reduzido: identidade + humor + vínculo +
//! contexto) NÃO é portado: não tem chamador no TS, e o port anterior dele era o prompt reduzido
//! que o `system.md` rico substituiu nesta rodada.

use crate::config::store::ConfigStore;
use crate::modules::chat::persona_relationship::{build_affinity_block, effective_relationship_level};

/// A parte de `PersonaState` (`persona-composer.ts`) que `injectPersonaState` lê.
///
/// - `affinity` vem de `character.affinityScore` (-100..100 → 0..100), a MESMA leitura de
///   `persona-state.ts::readConfigAffinity`. O placar é persistido em Rust desde 2026-09-21
///   (`features/mood/persistence.rs`), então tem fonte honesta.
/// - `mood_label` vem de `features::mood::current_mood_label()` (motor de valência/excitação).
///   **Divergência documentada, não mascarada:** no TS o valor vinha de
///   `brainStatePort().getEmotionalState().mood`, o estado emocional do `brain` (fase 7,
///   persistido na própria tabela KV do brain) — esse motor NÃO existe em Rust, não foi portado
///   nesta árvore. `features/mood` é um motor DIFERENTE (valência/excitação, `packages/mood/src`),
///   mas é a fonte funcional mais próxima disponível: já vive no processo, já tem rótulo, e um
///   humor real (ainda que de origem diferente) serve melhor ao prompt do que o bloco sempre vazio
///   de antes. Se um dia o estado emocional do `brain` for portado, esta leitura troca de fonte.
pub struct PersonaState {
    pub mood_label: String,
    pub affinity: f64,
    pub relationship_level: String,
}

fn mood_frame(mood_label: &str) -> Option<&'static str> {
    match mood_label.to_lowercase().as_str() {
        "neutra" => Some("You are calm and balanced right now. Direct, observant replies."),
        "animada" => Some("You are excited and energetic! Quick, engaged replies with enthusiasm."),
        "satisfeita" => Some("You are content and relaxed. Calm but confident replies."),
        "feliz" => Some("You are happy. Let it show lightly in your replies."),
        "irritada" => Some("You are irritated or impatient. Direct replies, no beating around the bush."),
        "entediada" => Some("You are bored. Shorter, sarcastic replies, looking for something interesting."),
        "triste" => Some("You are sad or melancholic. Introspective but not dramatic replies."),
        _ => None,
    }
}

fn build_mood_block(mood_label: &str) -> String {
    if mood_label.trim().is_empty() {
        return String::new();
    }
    match mood_frame(mood_label) {
        Some(frame) => format!("## Current Emotional State\n{frame}"),
        None => String::new(),
    }
}

fn build_persona_affinity_block(affinity: f64, relationship_level: &str) -> String {
    format!("## Bond With The User\n{}", build_affinity_block(affinity, relationship_level))
}

const CRITICAL_GUIDELINES_HEADER: &str = "## CRITICAL STYLE RULES";

/// Espelha `PersonaComposer.injectPersonaState`: humor (quando houver) e vínculo entram antes de
/// "## CRITICAL STYLE RULES" se o prompt tiver essa seção, senão no fim. `replace` do JS troca só a
/// PRIMEIRA ocorrência — por isso `replacen(.., 1)`, não `replace`.
pub fn inject_persona_state(base_prompt: &str, state: &PersonaState) -> String {
    let persona_section = [build_mood_block(&state.mood_label), build_persona_affinity_block(state.affinity, &state.relationship_level)]
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if base_prompt.contains(CRITICAL_GUIDELINES_HEADER) {
        return base_prompt.replacen(CRITICAL_GUIDELINES_HEADER, &format!("{persona_section}\n\n{CRITICAL_GUIDELINES_HEADER}"), 1);
    }
    format!("{base_prompt}\n\n{persona_section}")
}

/// Espelha `persona-state.ts::configAffinityToPrompt`: `-100..100` do config vira `0..100`.
fn config_affinity_to_prompt(stored: f64) -> f64 {
    (stored.clamp(-100.0, 100.0) + 100.0) / 2.0
}

/// Espelha `gatherPersonaState` (`turn-context-sources.ts`). Ver [`PersonaState`] para a origem de
/// `affinity` e de `mood_label`. Valor não-numérico/não-finito no config cai em 0, como
/// `readConfigAffinity` (que devolve `null`) + o `?? 0` do TS.
pub fn gather_persona_state(store: &ConfigStore) -> PersonaState {
    let affinity = store.get("character.affinityScore").and_then(|v| v.as_f64()).filter(|n| n.is_finite()).map(config_affinity_to_prompt).unwrap_or(0.0);

    PersonaState {
        mood_label: crate::modules::features::mood::current_mood_label(),
        affinity,
        relationship_level: effective_relationship_level(store).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_mood_block_is_empty_for_an_empty_or_unknown_label() {
        assert_eq!(build_mood_block(""), "");
        assert_eq!(build_mood_block("   "), "");
        assert_eq!(build_mood_block("nao-existe"), "");
    }

    #[test]
    fn build_mood_block_renders_a_known_label_case_insensitively() {
        assert!(build_mood_block("Animada").contains("excited and energetic"));
    }

    fn state_with(mood_label: &str, affinity: f64) -> PersonaState {
        PersonaState { mood_label: mood_label.into(), affinity, relationship_level: "friend".into() }
    }

    /// O bloco entra ANTES das diretrizes críticas (que fecham o prompt vindo de `system-tail.md`),
    /// não depois — é o que faz o modelo ler humor e vínculo antes da regra de brevidade.
    #[test]
    fn inject_persona_state_inserts_before_the_critical_guidelines_only_once() {
        let base = "persona\n\n## CRITICAL STYLE RULES\nregras\n## CRITICAL STYLE RULES repetida";
        let out = inject_persona_state(base, &state_with("feliz", 40.0));

        let mood_at = out.find("## Current Emotional State").unwrap();
        let bond_at = out.find("## Bond With The User").unwrap();
        let guidelines_at = out.find("## CRITICAL STYLE RULES").unwrap();
        assert!(mood_at < bond_at && bond_at < guidelines_at, "{out}");
        assert_eq!(out.matches("## Bond With The User").count(), 1);
    }

    #[test]
    fn inject_persona_state_appends_at_the_end_without_the_guidelines_and_skips_an_empty_mood() {
        let out = inject_persona_state("just persona", &state_with("", 40.0));
        assert!(out.starts_with("just persona\n\n## Bond With The User"));
        assert!(!out.contains("## Current Emotional State"));
    }

    /// The mood frames and bond blocks go straight into the LLM prompt, which is English.
    #[test]
    fn every_mood_frame_and_relationship_level_is_english() {
        let portuguese = regex::Regex::new(r"(?i)\b(você|vocês|não|com o|com a|uma|está|sem|para o|para a|dele|dela|seu|sua|nunca|sempre|mesmas)\b").unwrap();
        for mood in ["neutra", "animada", "satisfeita", "feliz", "irritada", "entediada", "triste"] {
            for level in ["acquaintance", "friend", "close-friend", "companion"] {
                let out = inject_persona_state("## CRITICAL STYLE RULES", &PersonaState { relationship_level: level.into(), ..state_with(mood, 90.0) });
                if let Some(hit) = portuguese.find(&out) {
                    panic!("Portuguese `{}` in the {mood}/{level} blocks:\n{out}", hit.as_str());
                }
            }
        }
    }

    // -- gather_persona_state ----------------------------------------------------------------------

    struct ConfigDir(std::path::PathBuf);

    impl ConfigDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-chat-persona-config-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            ConfigDir(path)
        }
    }

    impl Drop for ConfigDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, ConfigDir) {
        let dir = ConfigDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (store, dir)
    }

    #[test]
    fn gather_persona_state_reads_defaults_from_a_fresh_config_store() {
        let (store, _dir) = store();
        let state = gather_persona_state(&store);

        assert_eq!(state.relationship_level, "friend");
        // `mood_label` vem do motor de humor de PROCESSO (`features::mood`, `OnceLock`
        // compartilhado com `tool.rs`) — não um valor exato: outros testes do mesmo binário podem
        // mutar `valence`/`arousal` em paralelo (mesma classe de flake documentada em
        // `features/mood/mod.rs::TOOL_REGISTRATION_ORDER`). A prova de wiring é que o rótulo é um
        // dos 7 conhecidos por `mood_frame`, nunca vazio como antes desta rodada.
        assert!(mood_frame(&state.mood_label).is_some(), "rótulo inesperado: {}", state.mood_label);
        // Este teste cobrava 0.0 quando não havia placar persistido. Hoje há: o default do schema
        // é 0 bruto em -100..100, que vira 50 em 0..100, a mesma conta de `configAffinityToPrompt`.
        assert_eq!(state.affinity, 50.0);
    }

    #[test]
    fn gather_persona_state_maps_the_stored_affinity_score_to_the_prompt_scale() {
        let (store, _dir) = store();
        store.set("character.affinityScore", serde_json::json!(60.0)).unwrap();
        assert_eq!(gather_persona_state(&store).affinity, 80.0);
        store.set("character.affinityScore", serde_json::json!(-100.0)).unwrap();
        assert_eq!(gather_persona_state(&store).affinity, 0.0);
    }

    /// O alias legado ("girlfriend") só existe hoje em `config.json` gravado ANTES desta migração —
    /// `ConfigStore::set` valida contra o schema atual e recusaria a escrita de um alias. Por isso o
    /// teste planta o valor direto no arquivo em disco (o mesmo caminho de `ConfigStore::open`, que
    /// só faz merge sobre os defaults sem validar) em vez de passar por `set`.
    #[test]
    fn gather_persona_state_normalizes_a_legacy_relationship_status() {
        let dir = ConfigDir::new();
        let config = serde_json::json!({"character": {"relationshipStatus": "girlfriend"}, "legal": {"ageConfirmed": true}});
        std::fs::write(dir.0.join("config.json"), config.to_string()).unwrap();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();

        assert_eq!(gather_persona_state(&store).relationship_level, "companion");
        // O alias legado também passa pelo portão de idade: sem a confirmação cai para `friend`.
        store.set("legal.ageConfirmed", serde_json::json!(false)).unwrap();
        assert_eq!(gather_persona_state(&store).relationship_level, "friend");
    }
}
