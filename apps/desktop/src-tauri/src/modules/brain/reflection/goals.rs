//! Porte MÍNIMO de `GoalsService.addGoal` (`services/brain/services/brain-goals.service.ts`) —
//! só o suficiente para [`super::run_reflection`] chamar depois de uma reflexão bem avaliada
//! (`Events.REFLECTED` → `addReflectionGoals` em `brain-event-handlers.ts`).
//!
//! **Lado de LEITURA — RESOLVIDO em rodada posterior**: `getBrainContext()` (TS) injeta `goals`
//! ativas no `dynamicContext` pull-based do turno de chat regular (via `ContextRegistry`, não o
//! prompt raw). [`get_active_goals`] agora tem chamador real:
//! `brain::persona::build_goals_context_block` (`persona/mod.rs`), no MESMO lugar que
//! `gaming_context`/`slang_context` já injetam — mesmo formato do TS (`getBrainContext`), 3 metas de
//! maior prioridade.
//!
//! `dedupeBy` de tags e o resto do CRUD (`addSubtask`/`completeGoal`/`updateGoal`/...) ficam de
//! fora: nenhum chamador em Rust os precisa ainda, e "mínimo" aqui significa só `addGoal`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::modules::memory::MemoryStore;

/// Chave de hash única para todas as metas — mesmo padrão de armazenamento que
/// `memory::social::store` usa para fatos (uma tabela lógica, um campo por `id`).
const GOALS_KEY: &str = "brain:goals";

/// = `SIMILARITY_THRESHOLD` (`brain.types.ts`): abaixo disso, uma observação nova vira meta nova.
const SIMILARITY_THRESHOLD: f64 = 0.6;

/// Porte reduzido de `GoalEntry` (`brain.types.ts`) — só os campos que `addGoal` preenche;
/// `completed`/`progress`/`subtasks`/`deadline`/`tags` do TS não têm chamador em Rust ainda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalEntry {
    pub id: String,
    pub description: String,
    pub priority: f64,
    pub created_at: i64,
}

/// Porte de `normalizeText` + `textSimilarity` (`brain-similarity.ts`): interseção sobre união dos
/// conjuntos de palavras (Jaccard), depois de baixar caixa e tirar pontuação.
fn text_similarity(a: &str, b: &str) -> f64 {
    let normalize = |text: &str| -> Vec<String> {
        text.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>().split_whitespace().map(str::to_string).collect()
    };
    let words_a: std::collections::HashSet<String> = normalize(a).into_iter().collect();
    let words_b: std::collections::HashSet<String> = normalize(b).into_iter().collect();
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.len() + words_b.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn read_goals(mem: &MemoryStore) -> Result<HashMap<String, GoalEntry>, String> {
    let raw = mem.h_get_all(GOALS_KEY)?;
    Ok(raw.into_iter().filter_map(|(id, json)| serde_json::from_str::<GoalEntry>(&json).ok().map(|g| (id, g))).collect())
}

/// Porte de `GoalsService.addGoal` (sem `tags`/`deadline`, sem chamador em Rust): existe meta
/// similar (`text_similarity >= SIMILARITY_THRESHOLD`) → só sobe a prioridade se a nova for maior;
/// senão → grava meta nova. Mesma regra de dedup do TS, sem o CRUD que ninguém chama ainda.
pub fn add_goal(mem: &MemoryStore, id: &str, description: &str, priority: f64, now_ms: i64) -> Result<(), String> {
    let mut goals = read_goals(mem)?;

    if let Some(existing) = goals.values_mut().find(|g| text_similarity(&g.description, description) >= SIMILARITY_THRESHOLD) {
        if priority > existing.priority {
            existing.priority = priority;
            let json = serde_json::to_string(existing).map_err(|e| e.to_string())?;
            let existing_id = existing.id.clone();
            mem.h_set(GOALS_KEY, &existing_id, &json)?;
        }
        return Ok(());
    }

    let entry = GoalEntry { id: id.to_string(), description: description.to_string(), priority, created_at: now_ms };
    let json = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    mem.h_set(GOALS_KEY, id, &json)
}

/// Todas as metas gravadas — sem filtro de `completed` (o campo não existe neste porte reduzido).
/// Chamada por `brain::persona::build_goals_context_block` a cada turno de chat (ver o cabeçalho do
/// arquivo), e pelos testes abaixo para provar que `add_goal` gravou de verdade.
pub fn get_active_goals(mem: &MemoryStore) -> Result<Vec<GoalEntry>, String> {
    let mut goals: Vec<GoalEntry> = read_goals(mem)?.into_values().collect();
    goals.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
    Ok(goals)
}

/// Apaga TODAS as metas — `del` sobre a MESMA chave que `h_set`/`h_get_all` usam, então nenhum
/// campo sobrevive. Usado por `features::privacy::clear` ("goals" deixou de estar `SKIPPED` nesta
/// rodada: a store existe desde `add_goal`, só faltava expor uma forma de limpar sem vazar
/// `GOALS_KEY` para fora deste módulo).
pub fn clear_goals(mem: &MemoryStore) -> Result<(), String> {
    mem.del(GOALS_KEY).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn clear_goals_removes_every_field_of_the_hash() {
        let mem = store();
        add_goal(&mem, "g1", "aprender rust", 0.5, 1000).unwrap();
        add_goal(&mem, "g2", "terminar o jogo", 0.9, 1000).unwrap();
        assert_eq!(get_active_goals(&mem).unwrap().len(), 2, "setup: as 2 metas precisam existir antes do clear");

        clear_goals(&mem).unwrap();

        assert!(get_active_goals(&mem).unwrap().is_empty(), "metas sobreviveram ao clear_goals");
    }

    #[test]
    fn text_similarity_e_1_para_frases_identicas() {
        assert_eq!(text_similarity("seja mais direta", "seja mais direta"), 1.0);
    }

    #[test]
    fn text_similarity_e_0_sem_nenhuma_palavra_em_comum() {
        assert_eq!(text_similarity("gosta de rpg", "detesta futebol americano"), 0.0);
    }

    #[test]
    fn text_similarity_ignora_pontuacao_e_caixa() {
        let a = text_similarity("Seja mais Direta!", "seja mais direta");
        assert_eq!(a, 1.0);
    }

    #[test]
    fn add_goal_grava_uma_meta_nova() {
        let mem = store();
        add_goal(&mem, "auto-reflect-1", "Improve based on self-reflection: ser mais direta", 6.0, 1000).unwrap();

        let goals = get_active_goals(&mem).unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].description, "Improve based on self-reflection: ser mais direta");
        assert_eq!(goals[0].priority, 6.0);
    }

    /// Mesma regra do TS: meta parecida existente não duplica, só sobe prioridade se a nova for maior.
    #[test]
    fn add_goal_funde_com_meta_parecida_em_vez_de_duplicar() {
        let mem = store();
        add_goal(&mem, "auto-reflect-1", "Improve based on self-reflection: ser mais direta", 4.0, 1000).unwrap();
        add_goal(&mem, "auto-reflect-2", "Improve based on self-reflection: ser mais direta e objetiva", 8.0, 2000).unwrap();

        let goals = get_active_goals(&mem).unwrap();
        assert_eq!(goals.len(), 1, "meta parecida não deveria duplicar");
        assert_eq!(goals[0].priority, 8.0, "prioridade maior da nova deveria vencer");
    }

    #[test]
    fn add_goal_nao_abaixa_prioridade_quando_a_nova_e_menor() {
        let mem = store();
        add_goal(&mem, "auto-reflect-1", "ser mais direta", 8.0, 1000).unwrap();
        add_goal(&mem, "auto-reflect-2", "ser mais direta", 2.0, 2000).unwrap();

        let goals = get_active_goals(&mem).unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].priority, 8.0);
    }

    #[test]
    fn add_goal_grava_metas_distintas_separadamente() {
        let mem = store();
        add_goal(&mem, "auto-reflect-1", "ser mais direta", 6.0, 1000).unwrap();
        add_goal(&mem, "auto-reflect-2", "falar menos sobre política", 7.0, 2000).unwrap();

        let goals = get_active_goals(&mem).unwrap();
        assert_eq!(goals.len(), 2);
    }
}
