//! Fusão de campos — porte de
//! `apps/desktop/src/features/person-catalog/services/person-merge.ts`. As funções que
//! [`super::mutate`] usa no caminho de `upsert`/`add_mention`/`delete` (as 11 ações da tool) MAIS
//! [`repoint_orphan_relationships`], que [`super::mutate::merge`] usa para `person_catalog_merge`
//! (RPC de `mika:plugin-rpc`, não ação da tool). `autoMergeExactDuplicates`/`purgeNonPersonRecords`
//! continuam não portados — chamados só no boot do plugin TS (`index.ts::activate`), fora tanto das
//! 11 ações da tool quanto dos 10 métodos RPC.

use super::normalize::normalize;
use super::types::{PersonRelationship, RelationshipType};

/// Porte de `supersede_pair` (memory/social/relationship_graph.rs) para a cópia denormalizada que o
/// prompt lê: uma correção ("Maria: colega" -> "Maria: prima") tem de SUBSTITUIR a entrada existente
/// para o mesmo `name`, não empilhar uma segunda — senão o cap de `people_block.rs` (`take(2)`,
/// mais antiga primeiro) pode descartar a correção e falar a versão obsoleta.
pub(super) fn merge_relationships_replace_by_name(existing: &[PersonRelationship], incoming: &[PersonRelationship]) -> Vec<PersonRelationship> {
    let mut merged: Vec<PersonRelationship> = existing.to_vec();
    for rel in incoming {
        if let Some(slot) = merged.iter_mut().find(|r| r.name == rel.name) {
            *slot = rel.clone();
        } else {
            merged.push(rel.clone());
        }
    }
    merged
}

fn fact_tokens(text: &str) -> std::collections::HashSet<String> {
    text.split(' ').filter(|w| w.len() > 3).map(str::to_string).collect()
}

/// Porte de `isSimilarFact` — linha igual (após normalizar) OU sobreposição de tokens >= 60% do
/// menor conjunto.
fn is_similar_fact(line: &str, incoming_normalized: &str, incoming_tokens: &std::collections::HashSet<String>) -> bool {
    let normalized = normalize(line);
    if normalized == incoming_normalized {
        return true;
    }
    let tokens = fact_tokens(&normalized);
    if tokens.is_empty() || incoming_tokens.is_empty() {
        return false;
    }
    let intersection = tokens.iter().filter(|t| incoming_tokens.contains(*t)).count();
    let smaller = tokens.len().min(incoming_tokens.len());
    (intersection as f64 / smaller as f64) >= 0.6
}

/// Porte de `mergeBio` — cada linha vira um bullet `- `; a linha entrante só é ADICIONADA se
/// nenhuma linha existente for "similar" (near-duplicate por sobreposição de tokens), e a lista fica
/// presa nas últimas 25 linhas.
pub(super) fn merge_bio(existing: Option<&str>, incoming: &str) -> String {
    let mut lines: Vec<String> = existing
        .unwrap_or("")
        .split('\n')
        .map(|l| l.trim_start_matches('-').trim_start().trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let normalized_incoming = normalize(incoming);
    let incoming_tokens = fact_tokens(&normalized_incoming);
    let already_present = lines.iter().any(|l| is_similar_fact(l, &normalized_incoming, &incoming_tokens));
    if !incoming.is_empty() && !already_present {
        lines.push(incoming.to_string());
    }

    let start = lines.len().saturating_sub(25);
    lines[start..].iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
}

/// Porte de `mergeStringArray` — união preservando a ordem de primeira aparição (mesma semântica de
/// `new Set([...existing, ...incoming])`).
pub(super) fn merge_string_array(existing: Option<&[String]>, incoming: Option<&[String]>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for value in existing.unwrap_or(&[]).iter().chain(incoming.unwrap_or(&[]).iter()) {
        if seen.insert(value.clone()) {
            out.push(value.clone());
        }
    }
    out
}

/// Porte de `resolveRelationshipType` — `creator`/`vvip` são "travados" (não regridem por
/// afinidade), o resto é uma escada de limiares. `plays_with` (specs/08-gamer-companion.md) entra na
/// MESMA trava: uma vez atribuído pelo detector de parceiro de jogo recorrente
/// (`gamer::game_partners`), uma queda de afinidade não pode apagar o rótulo.
pub(super) fn resolve_relationship_type(affinity: f64, current: Option<RelationshipType>) -> RelationshipType {
    match current {
        Some(RelationshipType::Creator) => return RelationshipType::Creator,
        Some(RelationshipType::Vvip) => return RelationshipType::Vvip,
        Some(RelationshipType::PlaysWith) => return RelationshipType::PlaysWith,
        _ => {}
    }
    if affinity >= 90.0 {
        RelationshipType::BestFriend
    } else if affinity >= 60.0 {
        RelationshipType::CloseFriend
    } else if affinity >= 20.0 {
        RelationshipType::Friend
    } else if affinity >= 5.0 {
        RelationshipType::Acquaintance
    } else if affinity <= -50.0 {
        RelationshipType::Enemy
    } else if affinity <= -20.0 {
        RelationshipType::Rival
    } else {
        RelationshipType::Stranger
    }
}

/// Porte de `deadNames` — nomes normalizados de um registro apagado, para varrer relações órfãs.
pub(super) fn dead_names(names: &[String]) -> std::collections::HashSet<String> {
    names.iter().map(|n| normalize(n)).filter(|n| !n.is_empty()).collect()
}

/// Porte de `removeOrphanRelationships` — tira, de TODOS os outros registros, qualquer relação que
/// aponte (por nome normalizado) para alguém apagado, para o LLM nunca ler o nome de um fantasma.
pub(super) fn remove_orphan_relationships(persons: &mut [super::types::PersonRecord], dead: &std::collections::HashSet<String>) {
    for person in persons.iter_mut() {
        let Some(relationships) = &person.relationships else { continue };
        let kept: Vec<PersonRelationship> = relationships.iter().filter(|r| !dead.contains(&normalize(&r.name))).cloned().collect();
        if kept.len() != relationships.len() {
            person.relationships = Some(kept);
        }
    }
}

/// Porte de `repointOrphanRelationships` — ao contrário de [`remove_orphan_relationships`] (usado no
/// `delete`), aqui a relação NÃO some: aponta para `replacement_name` (o registro sobrevivente do
/// merge), e o resultado é deduplicado por `nome normalizado:tipo` (mesmo `dedupeBy` do TS) para não
/// deixar duas relações idênticas após o repointing.
pub(super) fn repoint_orphan_relationships(persons: &mut [super::types::PersonRecord], dead: &std::collections::HashSet<String>, replacement_name: &str) {
    for person in persons.iter_mut() {
        let Some(relationships) = &person.relationships else { continue };
        let repointed: Vec<PersonRelationship> = relationships
            .iter()
            .cloned()
            .map(|mut r| {
                if dead.contains(&normalize(&r.name)) {
                    r.name = replacement_name.to_string();
                }
                r
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<PersonRelationship> = repointed
            .into_iter()
            .filter(|r| seen.insert(format!("{}:{}", normalize(&r.name), r.kind)))
            .collect();
        person.relationships = Some(deduped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_bio_appends_a_new_distinct_line_as_a_bullet() {
        let merged = merge_bio(Some("- gosta de café"), "trabalha com música");
        assert_eq!(merged, "- gosta de café\n- trabalha com música");
    }

    #[test]
    fn merge_bio_skips_a_near_duplicate_line() {
        let merged = merge_bio(Some("- trabalha com desenvolvimento de software"), "trabalha com desenvolvimento de sistemas");
        assert_eq!(merged.lines().count(), 1, "linha quase idêntica devia ter sido descartada: {merged}");
    }

    #[test]
    fn merge_bio_keeps_only_the_last_25_lines() {
        // Vocabulário sem sobreposição de tokens (>3 chars) com a linha entrante — senão
        // `isSimilarFact` (>=60% de interseção do menor conjunto) descarta a entrante como
        // quase-duplicata da última linha existente em vez de truncar por tamanho.
        let existing = (0..25).map(|i| format!("- linha exclusiva {i} arquivada")).collect::<Vec<_>>().join("\n");
        let merged = merge_bio(Some(&existing), "detalhe adicional chegando agora");
        assert_eq!(merged.lines().count(), 25);
        assert!(merged.contains("detalhe adicional chegando agora"));
        assert!(!merged.contains("linha exclusiva 0 arquivada"));
    }

    #[test]
    fn merge_string_array_dedupes_preserving_first_seen_order() {
        let existing = vec!["a".to_string(), "b".to_string()];
        let incoming = vec!["b".to_string(), "c".to_string()];
        assert_eq!(merge_string_array(Some(&existing), Some(&incoming)), vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_relationship_type_locks_creator_regardless_of_affinity() {
        assert_eq!(resolve_relationship_type(-100.0, Some(RelationshipType::Creator)), RelationshipType::Creator);
    }

    #[test]
    fn resolve_relationship_type_locks_plays_with_regardless_of_affinity() {
        assert_eq!(resolve_relationship_type(-100.0, Some(RelationshipType::PlaysWith)), RelationshipType::PlaysWith);
        assert_eq!(resolve_relationship_type(0.0, Some(RelationshipType::PlaysWith)), RelationshipType::PlaysWith);
    }

    #[test]
    fn resolve_relationship_type_climbs_the_affinity_ladder() {
        assert_eq!(resolve_relationship_type(95.0, None), RelationshipType::BestFriend);
        assert_eq!(resolve_relationship_type(0.0, None), RelationshipType::Stranger);
        assert_eq!(resolve_relationship_type(-60.0, None), RelationshipType::Enemy);
    }

    #[test]
    fn remove_orphan_relationships_strips_only_the_dead_name() {
        let mut person = super::super::types::minimal_person("p1", "Survivor");
        person.relationships = Some(vec![
            PersonRelationship { name: "Ghost".into(), kind: "friend".into(), affinity: None, trust: None, description: None, last_interaction_at: None },
            PersonRelationship { name: "Alive".into(), kind: "friend".into(), affinity: None, trust: None, description: None, last_interaction_at: None },
        ]);
        let mut persons = vec![person];
        let dead = dead_names(&["Ghost".to_string()]);
        remove_orphan_relationships(&mut persons, &dead);
        let names: Vec<&str> = persons[0].relationships.as_ref().unwrap().iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Alive"]);
    }

    #[test]
    fn repoint_orphan_relationships_renames_to_the_survivor_and_dedupes() {
        let mut person = super::super::types::minimal_person("p1", "Watcher");
        person.relationships = Some(vec![
            PersonRelationship { name: "Old Name".into(), kind: "friend".into(), affinity: None, trust: None, description: None, last_interaction_at: None },
            // Já existia uma relação com o sobrevivente, mesmo tipo — tem de sobrar só uma.
            PersonRelationship { name: "Survivor".into(), kind: "friend".into(), affinity: None, trust: None, description: None, last_interaction_at: None },
        ]);
        let mut persons = vec![person];
        let dead = dead_names(&["Old Name".to_string()]);
        repoint_orphan_relationships(&mut persons, &dead, "Survivor");
        let names: Vec<&str> = persons[0].relationships.as_ref().unwrap().iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Survivor"], "repoint + dedupe devia deixar uma relação só");
    }
}
