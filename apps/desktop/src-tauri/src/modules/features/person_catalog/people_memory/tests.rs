//! The user's scenarios, end to end through the two boundaries the rest of the app uses:
//! [`apply_ops`] (post-turn extraction writes) and [`prompt_context`] (what the next turn reads).

use super::super::types::{PersonCatalogIndex, PersonRecord};
use super::*;

const T0: i64 = 1_000_000;

fn ids() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    format!("pc_t{}", N.fetch_add(1, Ordering::Relaxed))
}

fn apply(index: &mut PersonCatalogIndex, ops_json: &str, text: &str, now: i64) -> ApplyReport {
    let ops = parse_ops(ops_json);
    apply_ops(index, &ops, &ApplyCtx { text, now, gen_id: ids, max_mentions: 100 })
}

fn only(index: &PersonCatalogIndex) -> &PersonRecord {
    assert_eq!(index.persons.len(), 1, "expected exactly one person: {:?}", index.persons.iter().map(|p| &p.primary_name).collect::<Vec<_>>());
    &index.persons[0]
}

fn by_name<'a>(index: &'a PersonCatalogIndex, name: &str) -> &'a PersonRecord {
    index.persons.iter().find(|p| p.primary_name == name).unwrap_or_else(|| panic!("{name} not catalogued"))
}

fn fact_texts(p: &PersonRecord) -> Vec<&str> {
    p.facts.iter().map(|f| f.text.as_str()).collect()
}

/// "minha esposa gosta de sorvete" → a wife placeholder with the fact; "minha esposa se chama
/// Dani" → the SAME entity is named Dani, keeps the fact, and answers to "Esposa".
#[test]
fn wife_fact_first_then_her_name_updates_the_same_person() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"esposa"},"fact":"gosta de sorvete","confidence":0.9}]}"#, "minha esposa gosta de sorvete", T0);

    let wife = only(&index);
    assert!(wife.unnamed);
    assert_eq!(wife.primary_name, "Esposa");
    assert_eq!(wife.relation_to_user.as_deref(), Some("esposa"));
    assert_eq!(fact_texts(wife), vec!["gosta de sorvete"]);
    let id = wife.id.clone();

    apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"relation":"esposa"},"name":"Dani"}]}"#, "minha esposa se chama Dani", T0 + 1);

    let dani = only(&index);
    assert_eq!(dani.id, id, "naming must update the placeholder, not create a second wife");
    assert_eq!(dani.primary_name, "Dani");
    assert!(!dani.unnamed);
    assert!(dani.aliases.iter().any(|a| a == "Esposa"), "the relation label stays as an alias: {:?}", dani.aliases);
    assert_eq!(fact_texts(dani), vec!["gosta de sorvete"]);
}

/// English input lands on the same canonical relation as pt-BR.
#[test]
fn english_input_my_wife_likes_ice_cream_is_the_same_wife() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"wife"},"fact":"likes ice cream","confidence":0.9}]}"#, "my wife likes ice cream", T0);
    apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"relation":"esposa"},"name":"Dani"}]}"#, "minha esposa se chama Dani", T0 + 1);

    let dani = only(&index);
    assert_eq!(dani.primary_name, "Dani");
    assert_eq!(fact_texts(dani), vec!["likes ice cream"]);
}

/// After naming, "a Dani" and "minha esposa" both pull the same card into the next turn.
#[test]
fn alias_and_relation_lookup_find_the_same_person() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"esposa"},"fact":"gosta de sorvete"}]}"#, "minha esposa gosta de sorvete", T0);
    apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"relation":"esposa"},"name":"Dani"}]}"#, "ela se chama Dani", T0 + 1);
    let id = only(&index).id.clone();

    let by_alias = prompt_context(&index, "a Dani chegou em casa", 3);
    assert_eq!(by_alias.people.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec![id.as_str()]);
    let by_relation = prompt_context(&index, "Minha esposa tá cansada", 3);
    assert_eq!(by_relation.people.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec![id.as_str()]);
    assert!(by_relation.ambiguities.is_empty());

    // A later fact through the relation or the name reaches the same entity.
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"name":"dani"},"fact":"trabalha de casa"}]}"#, "a dani trabalha de casa", T0 + 2);
    assert_eq!(fact_texts(only(&index)), vec!["gosta de sorvete", "trabalha de casa"]);
}

/// A named person catalogued before the relation was known merges with the wife placeholder.
#[test]
fn a_named_person_merges_with_the_placeholder_when_the_relation_links_them() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"esposa"},"fact":"gosta de sorvete"}]}"#, "minha esposa gosta de sorvete", T0);
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"name":"Dani"},"fact":"joga Stardew Valley"}]}"#, "a Dani joga Stardew Valley", T0 + 1);
    assert_eq!(index.persons.len(), 2, "no link yet: Dani could be anyone");

    apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"relation":"esposa"},"name":"Dani"}]}"#, "minha esposa é a Dani", T0 + 2);

    let dani = only(&index);
    assert_eq!(dani.primary_name, "Dani");
    assert_eq!(dani.relation_to_user.as_deref(), Some("esposa"));
    let mut facts = fact_texts(dani);
    facts.sort_unstable();
    assert_eq!(facts, vec!["gosta de sorvete", "joga Stardew Valley"]);
}

fn two_brothers() -> PersonCatalogIndex {
    let mut index = PersonCatalogIndex::default();
    apply(
        &mut index,
        r#"{"ops":[{"op":"upsert_person","ref":{"name":"Pedro","relation":"irmão"}},{"op":"upsert_person","ref":{"name":"João","relation":"irmão"}}]}"#,
        "meus irmãos são o Pedro e o João",
        T0,
    );
    assert_eq!(index.persons.len(), 2);
    index
}

/// Two brothers + "meu irmão …" → nothing is guessed: the fact waits in a clarification, and the
/// turn context tells Mika to ask which one.
#[test]
fn two_brothers_turn_a_relation_only_fact_into_a_clarification() {
    let mut index = two_brothers();
    let report = apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#, "meu irmão passou na faculdade", T0 + 1);

    assert_eq!(report.clarifications_opened, 1);
    assert!(index.persons.iter().all(|p| p.facts.is_empty()), "the fact must not be attached to a guessed brother");
    let pending = &index.pending_clarifications[0];
    assert_eq!(pending.relation, "brother");
    assert_eq!(pending.facts, vec!["passou na faculdade".to_string()]);
    assert_eq!(pending.candidate_ids.len(), 2);

    let context = prompt_context(&index, "meu irmão passou na faculdade", 3);
    assert_eq!(context.ambiguities.len(), 1);
    let mut names = context.ambiguities[0].candidates.clone();
    names.sort_unstable();
    assert_eq!(names, vec!["João".to_string(), "Pedro".to_string()]);
    assert!(pending.asked, "this very reply was already told to ask — the next one must not ask again");
}

/// The user's answer ("o Pedro") resolves the clarification onto that brother.
#[test]
fn the_answer_resolves_the_pending_clarification() {
    let mut index = two_brothers();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#, "meu irmão passou na faculdade", T0 + 1);

    let report = apply(&mut index, r#"{"ops":[]}"#, "o Pedro", T0 + 2);

    assert_eq!(report.clarifications_resolved, 1);
    assert!(index.pending_clarifications.is_empty());
    assert_eq!(fact_texts(by_name(&index, "Pedro")), vec!["passou na faculdade"]);
    assert!(by_name(&index, "João").facts.is_empty());
}

/// An answer naming a brother nobody knew yet creates him and hands him the waiting fact.
#[test]
fn an_answer_naming_a_new_brother_creates_him_with_the_fact() {
    let mut index = two_brothers();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#, "meu irmão passou na faculdade", T0 + 1);

    apply(&mut index, r#"{"ops":[{"op":"upsert_person","ref":{"name":"Lucas","relation":"irmão"}}]}"#, "nenhum dos dois, o Lucas, meu outro irmão", T0 + 2);

    assert!(index.pending_clarifications.is_empty());
    assert_eq!(fact_texts(by_name(&index, "Lucas")), vec!["passou na faculdade"]);
}

/// One unnamed brother is named in place, and his relation word is not kept as an alias (a
/// second brother would inherit every "meu irmão" otherwise).
#[test]
fn a_single_unnamed_brother_is_named_without_keeping_a_multi_relation_alias() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"joga bola"}]}"#, "meu irmão joga bola", T0);
    apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"relation":"irmão"},"name":"Pedro"}]}"#, "meu irmão se chama Pedro", T0 + 1);

    let pedro = only(&index);
    assert_eq!(pedro.primary_name, "Pedro");
    assert!(pedro.aliases.is_empty(), "'Irmão' must not become Pedro's alias: {:?}", pedro.aliases);
    assert_eq!(fact_texts(pedro), vec!["joga bola"]);

    // A differently named brother is someone else; then "meu irmão" alone is ambiguous.
    apply(&mut index, r#"{"ops":[{"op":"upsert_person","ref":{"name":"João","relation":"irmão"}}]}"#, "meu irmão João", T0 + 2);
    assert_eq!(index.persons.len(), 2);
    let report = apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"gosta de pizza"}]}"#, "meu irmão gosta de pizza", T0 + 3);
    assert_eq!(report.clarifications_opened, 1);
}

/// The same extraction applied twice changes nothing the second time.
#[test]
fn applying_the_same_ops_twice_is_idempotent() {
    let mut index = PersonCatalogIndex::default();
    let ops = r#"{"ops":[{"op":"add_fact","ref":{"relation":"esposa"},"fact":"gosta de sorvete"}]}"#;
    apply(&mut index, ops, "minha esposa gosta de sorvete", T0);
    apply(&mut index, ops, "minha esposa gosta de sorvete", T0 + 1);
    assert_eq!(fact_texts(only(&index)), vec!["gosta de sorvete"]);
}

/// Conservative parsing: a malformed item is skipped without losing the valid ones, and a
/// low-confidence fact is never stored.
#[test]
fn malformed_items_and_low_confidence_facts_are_dropped() {
    let mut index = PersonCatalogIndex::default();
    let raw = r#"```json
    {"ops":[{"op":"teleport"},{"op":"add_fact","ref":{},"fact":"sem dono"},{"op":"add_fact","ref":{"relation":"mãe"},"fact":"talvez goste de chá","confidence":0.3},
    {"op":"add_fact","ref":{"relation":"mãe"},"fact":"mora em Recife","confidence":0.95}]}
    ```"#;
    assert_eq!(parse_ops(raw).len(), 3, "the unknown op is skipped, the rest parse");
    apply(&mut index, raw, "minha mãe mora em Recife", T0);
    assert_eq!(fact_texts(only(&index)), vec!["mora em Recife"]);
}

/// LLM output is external data: a `]` before the first `[` used to build the inverted slice
/// `raw[start..=end]` and panic the whole extraction.
#[test]
fn a_closing_bracket_before_the_first_opening_one_is_not_a_panic() {
    assert!(parse_ops("] sem lista [").is_empty());
    assert!(parse_ops("nota ] e depois [ texto solto").is_empty());
}

/// Hours later, a message that happens to name a candidate is not an answer anymore.
#[test]
fn a_late_mention_does_not_settle_an_old_question() {
    let mut index = two_brothers();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#, "meu irmão passou na faculdade", T0 + 1);
    apply(&mut index, r#"{"ops":[]}"#, "o Pedro vem jantar", T0 + 1 + 3 * 3_600_000);
    assert_eq!(index.pending_clarifications.len(), 1);
    assert!(by_name(&index, "Pedro").facts.is_empty());
}

/// Stale clarifications expire instead of haunting every later turn.
#[test]
fn a_clarification_expires() {
    let mut index = two_brothers();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#, "meu irmão passou na faculdade", T0 + 1);
    apply(&mut index, r#"{"ops":[]}"#, "bom dia", T0 + 1 + PENDING_TTL_MS + 1);
    assert!(index.pending_clarifications.is_empty());
}

/// Review 2026-09-28: renaming onto an existing name merged the two but reported the absorbed
/// (deleted) id as touched — the chat mention was lost and the survivor never marked.
#[test]
fn renaming_onto_an_existing_person_touches_the_survivor() {
    let mut index = PersonCatalogIndex::default();
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"name":"Pedrinho"},"fact":"joga truco"}]}"#, "o Pedrinho joga truco", T0);
    apply(&mut index, r#"{"ops":[{"op":"add_fact","ref":{"name":"Pedro"},"fact":"mora em Curitiba"}]}"#, "o Pedro mora em Curitiba", T0 + 1);
    let report = apply(&mut index, r#"{"ops":[{"op":"set_name","ref":{"name":"Pedrinho"},"name":"Pedro"}]}"#, "o Pedrinho é o Pedro", T0 + 2);

    let pedro = only(&index);
    assert_eq!(report.touched, vec![pedro.id.clone()]);
}
