//! Consulta do grafo de fatos além de pessoas — extraído de `mod.rs` (que ultrapassou o limite de
//! 300 linhas do AGENTS.md ao ganhar esta ação nesta rodada) para manter um arquivo por
//! responsabilidade, mesmo padrão de `fact_store.rs`/`rules.rs`/`store.rs` neste diretório.

use super::rules::{normalize_text, resolve_self_subject};
use super::types::{LearnedFact, SOCIAL_TABLE_FACTS};
use super::{fact_store, store};
use crate::modules::memory::MemoryStore;

/// = `SocialMemoryService::query_facts` — defects.md "memory-relationship" P2, "no real graph
/// beyond people": o caminho que deixa "do que eu gosto?"/"qual jogo eu jogo?" bater de forma
/// confiável com o que `add_fact` já gravou.
/// - `subject` passa por [`resolve_self_subject`], então uma tool que pede fatos do "usuario"/"eu"
///   cai no MESMO balde canônico que `add_fact` já usa como default — sem isto a consulta e a
///   escrita usariam subjects diferentes e nunca bateriam.
/// - `predicate`, quando informado, filtra por igualdade de [`normalize_text`] (mesma tolerância a
///   acento/caixa que o resto do módulo já aplica).
/// - Fato superseded (`active == Some(false)`) nunca aparece: uma correção via `add_fact` já
///   aposenta o antigo, e uma consulta ingênua não pode devolver os dois lados resolvidos de uma
///   contradição.
pub(super) fn query_facts(mem: &MemoryStore, subject: Option<&str>, predicate: Option<&str>, limit: usize) -> Result<Vec<LearnedFact>, String> {
    let scoped: Vec<LearnedFact> = match subject {
        Some(subject) => {
            let resolved = resolve_self_subject(subject);
            store::get_recent_by_subject(mem, SOCIAL_TABLE_FACTS, &resolved, fact_store::SOCIAL_SCAN_LIMIT)?
        }
        None => store::get_all(mem, SOCIAL_TABLE_FACTS)?,
    };
    let wanted_predicate = predicate.map(normalize_text);
    let filtered = scoped
        .into_iter()
        .filter(|f| f.active != Some(false))
        .filter(|f| match &wanted_predicate {
            None => true,
            Some(wanted) => f.predicate.as_deref().map(normalize_text).as_deref() == Some(wanted.as_str()),
        })
        .take(limit)
        .collect();
    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AddFactInput, SocialMemoryService};
    use super::super::rules::USER_SUBJECT;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn query_facts_resolves_a_self_alias_to_the_same_bucket_add_fact_wrote_to() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap();

        let found = svc.query_facts(Some("eu"), None, 10).unwrap();
        assert_eq!(found.len(), 1, "\"eu\" tem de resolver para o MESMO subject que add_fact usou por default");
        assert_eq!(found[0].fact, "gosta de rust");
    }

    #[test]
    fn query_facts_filters_by_normalized_predicate() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_fact(true, AddFactInput { fact: "joga elden ring".into(), predicate: Some("joga".into()), object: Some("elden ring".into()), ..Default::default() }, 1).unwrap();
        svc.add_fact(true, AddFactInput { fact: "gosta de rust".into(), predicate: Some("gosta de".into()), ..Default::default() }, 2).unwrap();

        let found = svc.query_facts(None, Some("Joga"), 10).unwrap();
        assert_eq!(found.len(), 1, "predicate compara por normalize_text — caixa não pode impedir o match");
        assert_eq!(found[0].object.as_deref(), Some("elden ring"));
    }

    #[test]
    fn query_facts_excludes_a_fact_already_superseded_by_a_correction() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_fact(true, AddFactInput { fact: "gosta de café".into(), ..Default::default() }, 1).unwrap();
        svc.add_fact(true, AddFactInput { fact: "não gosta mais de café, prefere chá".into(), ..Default::default() }, 2).unwrap();

        let found = svc.query_facts(Some(USER_SUBJECT), None, 10).unwrap();
        assert_eq!(found.len(), 1, "o lado aposentado da correção nunca deveria aparecer numa consulta");
        assert!(found[0].fact.contains("chá"), "só o fato corrigido (chá) sobrevive à consulta");
    }

    #[test]
    fn query_facts_respects_the_limit() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        // Tópicos distintos, não um número isolado: `fact_tokens`/`core_tokens` descartam token de
        // 1 caractere, então variar só o dígito (ex.: "fato numero 0"/"numero 1") produz o MESMO
        // conjunto de tokens para as 5 entradas e `add_fact` as funde por quase-duplicata — o
        // teste teria só 1 fato para consultar, nunca 5.
        for (i, topic) in ["rust", "futebol", "anime", "jazz", "fotografia"].into_iter().enumerate() {
            svc.add_fact(true, AddFactInput { fact: format!("gosta de {topic}"), ..Default::default() }, i as i64 + 1).unwrap();
        }
        assert_eq!(svc.query_facts(None, None, 3).unwrap().len(), 3);
    }
}
