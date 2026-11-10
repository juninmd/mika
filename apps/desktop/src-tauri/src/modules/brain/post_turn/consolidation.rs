//! Porte de `consolidateMemoryIfNeeded` (`services/memory/consolidator.ts`): dispara
//! `EpisodicMemoryService::consolidate_old` (já portada e, desde esta rodada, `async` — ver o
//! cabeçalho de `memory::episodic::consolidation`) só nos turnos "pesados"
//! ([`super::cadence::HeavyCadence`]), e só quando o tier curto já passou do corte.
//!
//! `llm_fn` amarra o MESMO prompt do TS: `consolidation.md`, namespace `memory` (diretório IRMÃO de
//! `brain`, não um arquivo dentro dele — confirmado em `apps/desktop/out/prompts/memory/`), com
//! `{{language}}` = `app.locale` do `ConfigStore` (mesmo default `pt-BR` do TS quando a chave não
//! está setada). Formatação do lote (`"{i+1}. {texto recortado em 300}"`) igual a `consolidator.ts`.

use tauri::AppHandle;

use crate::config::store::ConfigStore;

use super::super::{prompt_loader::load_prompt_namespaced, turn};

/// = `CONSOLIDATION_THRESHOLD` (`pipeline.ts`): só tenta consolidar quando o tier curto já passou
/// disso — `consolidateOld` internamente ainda exige um lote CHEIO (`batch_size`) para agir.
const CONSOLIDATION_THRESHOLD: usize = 200;
/// = o `50` fixo que `pipeline.ts::runConsolidation` passa para `consolidateMemoryIfNeeded`.
const CONSOLIDATION_BATCH_SIZE: usize = 50;
const CONSOLIDATION_PROMPT_NAMESPACE: &str = "memory";
const CONSOLIDATION_PROMPT_KEY: &str = "consolidation";
/// = `t.slice(0, 300)` dentro do `llmFn` de `consolidator.ts`.
const ENTRY_SNIPPET_CHARS: usize = 300;

/// Porte de `texts.map((t, i) => \`${i + 1}. ${t.slice(0, 300)}\`).join('\n')`.
fn format_batch(entries: &[String]) -> String {
    entries
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let snippet: String = text.chars().take(ENTRY_SNIPPET_CHARS).collect();
            format!("{}. {snippet}", i + 1)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Só roda em turno pesado (chamador já filtra) — tenta consolidar e nunca propaga erro: uma
/// consolidação falha não pode derrubar o listener que processa os PRÓXIMOS turnos, mesma política
/// de `consolidateMemoryIfNeeded` (`catch` + `log.warn`).
pub(super) async fn maybe_consolidate(app: &AppHandle, store: &ConfigStore) {
    let mem_store = match crate::modules::memory::memory_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::warn!("post_turn::consolidation: memória indisponível: {error}");
            return;
        }
    };
    let recording_allowed = crate::modules::memory::memory_recording_allowed(store);
    let service = crate::modules::memory::episodic_configured(mem_store);

    let short_total = match service.count_short() {
        Ok(count) => count,
        Err(error) => {
            log::warn!("post_turn::consolidation: falha ao contar tier curto: {error}");
            return;
        }
    };
    if short_total < CONSOLIDATION_THRESHOLD {
        return;
    }

    let language = crate::config::locale::app_language(store);
    let system_prompt = match load_prompt_namespaced(app, CONSOLIDATION_PROMPT_NAMESPACE, CONSOLIDATION_PROMPT_KEY, &std::collections::HashMap::from([("language".to_string(), language)])) {
        Ok(prompt) => prompt,
        Err(error) => {
            log::warn!("post_turn::consolidation: prompt indisponível: {error}");
            return;
        }
    };

    let removed = service
        .consolidate_old(recording_allowed, CONSOLIDATION_BATCH_SIZE, |entries| async move {
            let prompt = format_batch(&entries);
            let (_handle, signal) = turn::TurnAbortHandle::new();
            turn::run_turn_llm_raw(store, Some(&system_prompt), &prompt, signal, &turn::NullObserver)
                .await
                .map(|result| result.text.trim().to_string())
                .map_err(|error| error.to_string())
        })
        .await;

    match removed {
        Ok(0) => {}
        Ok(count) => log::info!("post_turn::consolidation: {count} entrada(s) consolidada(s) em 1 resumo"),
        Err(error) => log::warn!("post_turn::consolidation: falhou: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_batch_numera_e_recorta_cada_entrada_em_300_caracteres() {
        let long = "a".repeat(400);
        let batch = format_batch(&["fato curto".to_string(), long.clone()]);
        let lines: Vec<&str> = batch.split('\n').collect();
        assert_eq!(lines[0], "1. fato curto");
        assert_eq!(lines[1].chars().count(), "2. ".len() + ENTRY_SNIPPET_CHARS);
    }

    #[test]
    fn format_batch_vazio_devolve_string_vazia() {
        assert_eq!(format_batch(&[]), "");
    }
}
