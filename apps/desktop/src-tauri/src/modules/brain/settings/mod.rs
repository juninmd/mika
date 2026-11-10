//! Registro de settings de `mika.brain` no `FeatureRegistry` compartilhado — porte da parte de
//! settings de `registerFeatureWithTools('mika.brain', BrainFeature)` (`bootstrap/tools.ts`);
//! tools ficam em `super::tools`. Ver [`brain_tab`] para a tabela de campos da aba.
//!
//! **A aba `llm` NÃO mora aqui** — mudou de dono nesta rodada. `BrainFeature.settings =
//! [BRAIN_SETTINGS, LLM_SETTINGS]` no TS só AGREGA `LLM_SETTINGS`; quem AUTORA o schema é
//! `apps/desktop/src/main/services/llm/llm.schema.ts`. `modules/llm/` é dono único da aba `llm`
//! no `FeatureRegistry` (registra sob `pluginId: "mika.llm"`) — duas frentes registrando o mesmo
//! `tabId` faria o `FeatureRegistry` descartar uma em silêncio (dedupe por `tabId`, só um `warn`).
//! Coordenado em 2026-09-21; `llm_tab.rs` que existia aqui foi removido, não é regressão.

mod brain_tab;

use crate::feature_registry::FeatureRegistration;

/// Registra a aba `brain` — chame de `modules()` (efeito colateral, sem `ConfigStore`/`AppHandle`
/// nenhum: a declaração é dado estático).
pub(super) fn register() {
    crate::feature_registry::shared().register(
        "mika.brain",
        FeatureRegistration { enabled: true, settings: vec![brain_tab::declaration()], ..Default::default() },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register_publishes_the_brain_tab_under_mika_brain() {
        register();
        let all = crate::feature_registry::shared().get_all_settings();

        let brain = all.iter().find(|d| d["tabId"] == json!("brain")).expect("aba brain registrada");
        assert_eq!(brain["pluginId"], json!("mika.brain"));
    }

    /// Ancora a decisão de 2026-09-21: a aba `llm` não é mais registrada por `mika.brain` — se
    /// este teste falhar, a colisão de `tabId` com `modules/llm/` provavelmente voltou.
    #[test]
    fn does_not_publish_an_llm_tab_anymore() {
        register();
        let all = crate::feature_registry::shared().get_all_settings();
        assert!(
            !all.iter().any(|d| d["tabId"] == json!("llm") && d["pluginId"] == json!("mika.brain")),
            "mika.brain voltou a registrar a aba llm — dono único agora é modules/llm/"
        );
    }
}
