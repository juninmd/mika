//! `block`/`unblock`/`listBlocked` — porte de
//! `apps/desktop/src/features/person-catalog/services/person-catalog-moderation.ts`. Só usado por
//! `person_catalog_block`/`_unblock`/`_blocked_list` (RPC de `mika:plugin-rpc`), não pelas 11 ações
//! da tool — `mutate/mod.rs` documenta por quê estas três ficaram de fora daquela lista.

use super::super::normalize::catalog_key;
use super::super::query::find_person_by_alias;
use super::super::types::PersonCatalogIndex;
use super::lifecycle::delete_person;

pub struct BlockOutcome {
    pub blocked_keys: Vec<String>,
    pub removed: bool,
}

/// Porte de `blockPerson` — bloqueia a chave normalizada do nome E de cada alias (se a pessoa
/// existir no catálogo; senão bloqueia só o identificador cru, mesma regra do TS), e apaga o
/// registro catalogado. A entrada na lista de bloqueio sobrevive mesmo quando não havia registro
/// para apagar (`removed = false`), porque é ISSO que barra um upsert futuro com o mesmo nome.
pub(in super::super) fn block_person(index: &mut PersonCatalogIndex, identifier: &str) -> BlockOutcome {
    let person = index
        .persons
        .iter()
        .find(|p| p.id == identifier)
        .cloned()
        .or_else(|| find_person_by_alias(index, identifier).cloned());

    let names: Vec<String> = match &person {
        Some(p) => std::iter::once(p.primary_name.clone()).chain(p.aliases.iter().cloned()).collect(),
        None => vec![identifier.to_string()],
    };
    let keys: Vec<String> = names.iter().map(|n| catalog_key(n)).filter(|k| !k.is_empty()).collect();

    let mut blocked = index.blocked.clone().unwrap_or_default();
    for key in &keys {
        if !blocked.contains(key) {
            blocked.push(key.clone());
        }
    }
    index.blocked = Some(blocked);

    let removed = match &person {
        Some(p) => delete_person(index, &p.id).is_ok(),
        None => false,
    };

    BlockOutcome { blocked_keys: keys, removed }
}

/// Porte de `unblockPerson` — `Err` nomeado quando a chave não estava bloqueada (mesmo
/// `fail('Not blocked: ...')` do TS), nunca um sucesso silencioso para uma chave inexistente.
pub(in super::super) fn unblock_person(index: &mut PersonCatalogIndex, identifier: &str) -> Result<Vec<String>, String> {
    let key = catalog_key(identifier);
    let mut blocked = index.blocked.clone().unwrap_or_default();
    let before = blocked.len();
    blocked.retain(|b| b != &key);
    if blocked.len() == before {
        return Err(format!("Not blocked: {identifier}"));
    }
    index.blocked = Some(blocked.clone());
    Ok(blocked)
}

/// Porte de `PersonCatalogService.listBlocked` — leitura pura.
pub(in super::super) fn list_blocked(index: &PersonCatalogIndex) -> Vec<String> {
    index.blocked.clone().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::UpsertInput;
    use super::super::upsert::upsert_person;

    fn test_id() -> String {
        "pc_test1234".to_string()
    }

    fn empty_index() -> PersonCatalogIndex {
        PersonCatalogIndex { version: 2, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    #[test]
    fn blocking_a_cataloged_person_removes_the_record_and_blocks_name_and_aliases() {
        let mut idx = empty_index();
        let input = UpsertInput { name: "Ada Lovelace".into(), aliases: Some(vec!["Ada".into()]), ..Default::default() };
        let person = upsert_person(&mut idx, input, 1000, 100, test_id).person.unwrap();

        let outcome = block_person(&mut idx, &person.id);
        assert!(outcome.removed);
        assert!(idx.persons.is_empty());
        assert!(idx.blocked.as_ref().unwrap().contains(&"ada lovelace".to_string()));
        assert!(idx.blocked.as_ref().unwrap().contains(&"ada".to_string()));
    }

    #[test]
    fn blocking_an_identifier_never_cataloged_still_blocks_the_raw_key() {
        let mut idx = empty_index();
        let outcome = block_person(&mut idx, "Nunca Catalogado");
        assert!(!outcome.removed, "nada pra remover");
        assert!(idx.blocked.as_ref().unwrap().contains(&"nunca catalogado".to_string()));
    }

    #[test]
    fn unblock_reopens_the_door_for_a_future_upsert() {
        let mut idx = empty_index();
        idx.blocked = Some(vec!["ada".to_string()]);
        let blocked = unblock_person(&mut idx, "Ada").unwrap();
        assert!(blocked.is_empty());
    }

    #[test]
    fn unblock_an_identifier_that_was_never_blocked_is_a_named_error() {
        let mut idx = empty_index();
        let err = unblock_person(&mut idx, "ninguem").unwrap_err();
        assert!(err.contains("Not blocked"));
    }

    #[test]
    fn list_blocked_reflects_the_current_set() {
        let mut idx = empty_index();
        idx.blocked = Some(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(list_blocked(&idx), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn list_blocked_defaults_to_empty_when_nothing_was_ever_blocked() {
        assert!(list_blocked(&empty_index()).is_empty());
    }
}
