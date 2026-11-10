//! "conversation history" e "goals" — os 2 stores que deixam de estar em [`super::clear::SKIPPED_STORES`]
//! nesta rodada. Extraído de `clear.rs` só pelo limite de 300 linhas do arquivo (estava em 300,
//! ver o cabeçalho de lá), não por camada nova — mesmo motivo de `clear_ext.rs`.
//!
//! ## Por que estes dois, e por que agora
//!
//! A rodada de 2026-09-22 (`clear_ext.rs`) media "conversation history" e "goals and mood" como
//! sem store Rust alcançável. Isso ficou velho: `crate::modules::chat::clear_all_history`
//! (`chat/mod.rs`) e `crate::modules::brain::reflection::goals::clear_goals` (`goals.rs`) passaram
//! a existir depois — ver o cabeçalho de cada um para o porquê de não terem chamador de produção
//! antes desta rodada. "mood" continua `SKIPPED`: `character.affinityScore`
//! (`modules/features/mood/persistence.rs`) é um mecanismo ADITIVO separado do que o TS original
//! (`BrainService.clearGoalsAndMood`) de fato apagava (`this.emotional`, persistido numa tabela
//! `kv` própria do `PluginStore` de `brain` — ver `brain-emotional.ts`/`brain.service.ts` no
//! histórico do TS); nenhum Rust hoje tem essa tabela.

use crate::modules::memory::MemoryStore;

/// Espelha a metade "goals" de `clearGoalsAndMood`: `del` sobre a chave `brain:goals` inteira via
/// `brain::reflection::goals::clear_goals`, sem duplicar `GOALS_KEY` aqui fora.
pub(super) fn clear_goals(memory: &MemoryStore, cleared: &mut Vec<&'static str>, failed: &mut Vec<(&'static str, String)>) {
    match crate::modules::brain::reflection::goals::clear_goals(memory) {
        Ok(()) => cleared.push("goals"),
        Err(error) => failed.push(("goals", error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::brain::reflection::goals::{add_goal, get_active_goals};

    #[test]
    fn clear_goals_reports_cleared_and_actually_empties_the_store() {
        let mem = MemoryStore::open_in_memory().unwrap();
        add_goal(&mem, "g1", "aprender rust", 0.5, 1000).unwrap();

        let mut cleared = Vec::new();
        let mut failed = Vec::new();
        clear_goals(&mem, &mut cleared, &mut failed);

        assert!(failed.is_empty());
        assert_eq!(cleared, vec!["goals"]);
        assert!(get_active_goals(&mem).unwrap().is_empty(), "meta sobreviveu ao clear_goals");
    }
}
