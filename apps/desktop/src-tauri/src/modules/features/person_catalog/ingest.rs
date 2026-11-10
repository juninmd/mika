//! What to do with one [`ExtractionResult`]: drop references to the user and to Mika, apply the
//! people-memory ops to the catalog (`CatalogStore::apply_people_ops`), denormalize person↔person
//! links onto the source person for the recall card, and mirror identities + links into the social
//! graph (the canonical, bidirectional graph for `relationships`/`path`). `lifecycle.rs` owns the
//! listener/gate/config; this file owns only the writes.

use crate::modules::memory::social::{AddPersonToPersonInput, SocialMemoryService};
use crate::modules::memory::MemoryStore;

use super::mention_extract::{ExtractedRelationship, ExtractionResult};
use super::normalize::catalog_key;
use super::people_memory::{PersonOp, PersonRef};
use super::pii::redact_pii;
use super::store::CatalogStore;
use super::types::{PersonRelationship, UpsertInput};

/// Item 2 do pacote "people catalog + relationship graph": "Do not catalog the user themselves...
/// nor Mika herself". Um nome que bate com `character_name` (`character.name`, default "Mika") OU
/// com o nome que o PRÓPRIO texto usa para a auto-apresentação ("meu nome é X"/"me chamo X") nunca
/// vira um registro de estranho — a auto-apresentação já atualiza o perfil do usuário por outro
/// caminho (`brain::post_turn::social_ingest`, regex separada, mesmo turno); catalogar o MESMO nome
/// aqui como terceiro criaria a duplicata "usuário catalogado como estranho" que o smoke baseline
/// (item 9) flagrou.
pub(super) fn is_excluded_person(name: &str, source_text: &str, character_name: &str) -> bool {
    let key = catalog_key(name);
    if key == catalog_key(character_name) {
        return true;
    }
    self_intro_pattern().captures(source_text).and_then(|c| c.get(1)).is_some_and(|m| catalog_key(m.as_str().trim()) == key)
}

/// Cópia intencional de `brain::post_turn::social_ingest::name_pattern` — mesmo padrão de
/// duplicação já aceito nesta base entre núcleo/feature (ver `memory::social::fact_store::
/// redact_secrets` vs. `person_catalog::pii::redact_secrets`): `person_catalog` não pode importar
/// de `brain` (camada tardia não é dependência de outra tardia) só por 1 regex. Diverge do original
/// num ponto: a classe de caracteres não inclui mais espaço dentro de UM SÓ grupo `{1,40}` (isso
/// fazia o regex continuar capturando a frase inteira quando o nome não é seguido de pontuação —
/// "Meu nome é Antônio e eu adoro..." virava o nome inteiro "Antônio e eu adoro..." em vez de só
/// "Antônio"). Palavras adicionais só entram se TAMBÉM começarem maiúscula (nome composto tipo
/// "João Silva"), nunca uma palavra minúscula de continuação de frase.
///
/// `(?i)` é escopado SÓ à alternação do gatilho (`(?i:meu nome é|...)`), nunca ao padrão inteiro —
/// a crate `regex` faz "case folding" de QUALQUER classe Unicode sob `(?i)` global, inclusive
/// `\p{Lu}` (maiúscula), que passa a casar minúscula também. Um `(?i)` solto no início do padrão
/// destrava de novo o mesmo bug que o parágrafo acima diz ter corrigido: "Antônio e eu adoro"
/// inteiro voltava a ser capturado, porque `\p{Lu}` sob case-fold deixa de filtrar "e"/"eu"/"adoro"
/// como continuação. Confirmado por `is_excluded_person_excludes_a_name_the_message_uses_to_self_introduce`
/// (falhava com o `(?i)` global) e por teste isolado do padrão fora desta base.
fn self_intro_pattern() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r"\b(?i:meu nome é|me chamo|pode me chamar de)\s+(\p{Lu}[\p{L}\d'-]{0,20}(?:\s\p{Lu}[\p{L}\d'-]{0,20}){0,3})").unwrap())
}

/// As entradas de `person.relationships` (recall do catálogo, item 3) que dizem respeito a `name`
/// como ORIGEM de uma aresta extraída — só a origem, não o destino (ver o cabeçalho do módulo:
/// mesma assimetria do exemplo de aceitação da tarefa, onde só João exibe "trabalha com Maria", não
/// o inverso em Maria). O grafo social ([`SocialMemoryService`]) continua sendo a fonte BIDIRECIONAL
/// (BFS `path`/`relationships`); isto é só a cópia barata que `format_people_block` lê sem tocar o
/// `MemoryStore`.
fn relationships_as_source(name: &str, relationships: &[ExtractedRelationship]) -> Vec<PersonRelationship> {
    let key = catalog_key(name);
    relationships
        .iter()
        .filter(|r| catalog_key(&r.from) == key)
        .map(|r| PersonRelationship { name: r.to.clone(), kind: r.label.clone(), affinity: None, trust: None, description: None, last_interaction_at: None })
        .collect()
}

fn op_names(op: &PersonOp) -> Vec<&str> {
    let (target, extra) = match op {
        PersonOp::UpsertPerson { target, .. } | PersonOp::AddFact { target, .. } | PersonOp::AskClarification { target } => (target, None),
        PersonOp::SetName { target, name } => (target, Some(name.as_str())),
        PersonOp::AddAlias { target, alias } => (target, Some(alias.as_str())),
    };
    target.name.as_deref().into_iter().chain(extra).collect()
}

/// Applies one extraction. Pure enough to test without `AppHandle`: both stores come in open.
/// O texto do fato é saída do modelo sobre a frase do usuário: contato ou documento citado não vai cru para o catálogo.
fn redact_fact(op: &PersonOp) -> PersonOp {
    match op {
        PersonOp::AddFact { target, fact, confidence } => PersonOp::AddFact { target: target.clone(), fact: redact_pii(fact), confidence: *confidence },
        other => other.clone(),
    }
}

pub(super) fn ingest_extraction(
    catalog: &CatalogStore,
    social_mem: Option<&MemoryStore>,
    recording_allowed: bool,
    extracted: &ExtractionResult,
    source_text: &str,
    now: i64,
    max_mentions: usize,
    character_name: &str,
) {
    // `memory.enabled=false`: nada é gravado — nem a ficha, nem o trecho da frase, nem os vínculos.
    // Antes só a aresta pessoa↔pessoa respeitava o gate; a ficha entrava mesmo com a memória desligada.
    if !recording_allowed {
        return;
    }
    let ops: Vec<PersonOp> = extracted.ops.iter().filter(|op| !op_names(op).iter().any(|n| is_excluded_person(n, source_text, character_name))).map(redact_fact).collect();
    let report = match catalog.apply_people_ops(&ops, &redact_pii(source_text), now, max_mentions) {
        Ok(report) => report,
        Err(error) => {
            log::warn!("people memory: applying the extraction failed: {error}");
            return;
        }
    };
    let links: Vec<&ExtractedRelationship> =
        extracted.relationships.iter().filter(|r| !is_excluded_person(&r.from, source_text, character_name) && !is_excluded_person(&r.to, source_text, character_name)).collect();
    denormalize_links(catalog, &links, now, max_mentions);

    let Some(mem) = social_mem else { return };
    let svc = SocialMemoryService::new(mem);
    for person in report.touched.iter().filter_map(|id| catalog.get(id)).filter(|p| !p.unnamed) {
        if let Err(error) = svc.link_catalog_person(&person.primary_name, &person.id, now) {
            log::warn!("people memory: linking the social-graph identity failed: {error}");
        }
    }
    for rel in links {
        let input = AddPersonToPersonInput { from_name: rel.from.clone(), to_name: rel.to.clone(), label: rel.label.clone(), confidence: rel.confidence };
        if let Err(error) = svc.add_person_to_person(recording_allowed, input, now) {
            log::warn!("people memory: writing a person-to-person link failed: {error}");
        }
    }
}

/// The recall card shows "João — …; trabalha com Maria": copy each link onto its source person
/// (only when that person is already catalogued — a link never creates anyone).
fn denormalize_links(catalog: &CatalogStore, links: &[&ExtractedRelationship], now: i64, max_mentions: usize) {
    let owned: Vec<ExtractedRelationship> = links.iter().map(|r| (*r).clone()).collect();
    let mut sources: Vec<&str> = links.iter().map(|r| r.from.as_str()).collect();
    sources.dedup();
    for from in sources {
        let Some(person) = catalog.find_by_alias(from) else { continue };
        let relationships = relationships_as_source(from, &owned);
        let input = UpsertInput { id: Some(person.id.clone()), name: person.primary_name.clone(), relationships: Some(relationships), ..Default::default() };
        if let Err(error) = catalog.upsert(input, now, max_mentions) {
            log::warn!("people memory: denormalizing a link failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- exclusão de usuário/Mika (item 2) ----------------------------------------------------

    #[test]
    fn is_excluded_person_excludes_the_configured_character_name_case_insensitively() {
        assert!(is_excluded_person("mika", "oi Mika, tudo bem?", "Mika"));
        assert!(is_excluded_person("MIKA", "oi", "Mika"));
    }

    #[test]
    fn is_excluded_person_excludes_a_name_the_message_uses_to_self_introduce() {
        let text = "Oi Mika, tudo bem? Meu nome é Antônio e eu adoro jogar Counter-Strike.";
        assert!(is_excluded_person("Antônio", text, "Mika"), "auto-apresentação vira perfil do usuário, não estranho catalogado");
    }

    /// Regressão: um `(?i)` sem escopo no início do padrão faz a crate `regex` fazer case-fold de
    /// `\p{Lu}` também, deixando "e"/"eu"/"adoro" (minúsculas) casarem como "continuação
    /// maiúscula" — o padrão volta a capturar a frase inteira em vez de só o nome. Pega o defeito
    /// no nível do regex, não só via `is_excluded_person` (que também cobre o caso, mas mistura a
    /// causa com o efeito).
    #[test]
    fn self_intro_pattern_stops_the_capture_at_the_first_lowercase_word() {
        let text = "Oi Mika, tudo bem? Meu nome é Antônio e eu adoro jogar Counter-Strike.";
        let captured = self_intro_pattern().captures(text).and_then(|c| c.get(1)).map(|m| m.as_str());
        assert_eq!(captured, Some("Antônio"), "não pode engolir 'e eu adoro' como se fosse parte do nome");
    }

    /// O gatilho ("meu nome é"/"me chamo"/...) continua case-insensitive — só o `\p{Lu}` da
    /// captura precisa ficar de fora do fold, não a frase-gatilho inteira.
    #[test]
    fn self_intro_pattern_trigger_phrase_stays_case_insensitive() {
        let text = "MEU NOME É Bruno joga RPG";
        let captured = self_intro_pattern().captures(text).and_then(|c| c.get(1)).map(|m| m.as_str());
        assert_eq!(captured, Some("Bruno"));
    }

    #[test]
    fn is_excluded_person_does_not_exclude_an_unrelated_third_party_in_the_same_message() {
        let text = "Meu nome é Antônio, meu irmão é o João.";
        assert!(!is_excluded_person("João", text, "Mika"));
    }

    // --- relationships_as_source (recall do catálogo, item 3) ---------------------------------

    fn edge(from: &str, to: &str, label: &str) -> ExtractedRelationship {
        ExtractedRelationship { from: from.to_string(), to: to.to_string(), label: label.to_string(), confidence: None }
    }

    #[test]
    fn relationships_as_source_keeps_only_edges_where_name_is_the_origin() {
        let edges = vec![edge("João", "Maria", "trabalha com"), edge("Maria", "Pedro", "casada com")];
        let joao = relationships_as_source("João", &edges);
        assert_eq!(joao.len(), 1);
        assert_eq!(joao[0].name, "Maria");
        assert_eq!(joao[0].kind, "trabalha com");

        assert!(relationships_as_source("Pedro", &edges).is_empty(), "Pedro só aparece como destino, nunca como origem");
    }

    // --- ingest_extraction: o fluxo completo, item 1/2/3/4 juntos ------------------------------

    fn extracted_person(name: &str, relation_to_user: Option<&str>) -> PersonOp {
        let target = super::super::people_memory::PersonRef { name: Some(name.to_string()), relation: relation_to_user.map(str::to_string) };
        PersonOp::UpsertPerson { target, kind: Some("real_person".to_string()) }
    }

    fn open_catalog(dir: &TempDir) -> CatalogStore {
        CatalogStore::open(&dir.0.join("catalog.json")).unwrap()
    }

    /// O exemplo de aceitação DA TAREFA, ponta a ponta: "Meu irmão João trabalha com a Maria na
    /// padaria. A Maria é casada com o Pedro, que é o meu chefe." tem de produzir os 3 catalogados
    /// com o parentesco certo, o recall denormalizado certo (só a origem), E o grafo social
    /// navegável nos dois sentidos.
    #[test]
    fn ingest_extraction_matches_the_task_acceptance_example() {
        let dir = TempDir::new();
        let catalog = open_catalog(&dir);
        let mem = MemoryStore::open_in_memory().unwrap();
        let extracted = ExtractionResult {
            ops: vec![extracted_person("João", Some("irmão")), extracted_person("Maria", None), extracted_person("Pedro", Some("chefe"))],
            relationships: vec![edge("João", "Maria", "trabalha com"), edge("Maria", "Pedro", "casada com")],
        };
        let text = "Meu irmão João trabalha com a Maria na padaria do bairro. A Maria é casada com o Pedro, que é o meu chefe.";

        ingest_extraction(&catalog, Some(&mem), true, &extracted, text, 1, 100, "Mika");

        let joao = catalog.find_by_alias("João").expect("joão catalogado");
        assert_eq!(joao.relation_to_user.as_deref(), Some("irmão"));
        let joao_rels = joao.relationships.as_ref().expect("relationships denormalizadas");
        assert_eq!(joao_rels.len(), 1);
        assert_eq!(joao_rels[0].name, "Maria");
        assert_eq!(joao_rels[0].kind, "trabalha com");

        let pedro = catalog.find_by_alias("Pedro").expect("pedro catalogado");
        assert_eq!(pedro.relation_to_user.as_deref(), Some("chefe"));
        assert!(pedro.relationships.as_ref().map_or(true, Vec::is_empty), "pedro só aparece como destino, não deve ter relationships próprias");

        // Grafo social: BFS acha a conexão nos dois sentidos, e a identidade única liga o pc_* ao
        // registro social (item 2).
        let svc = SocialMemoryService::new(&mem);
        let maria_edges = svc.relationships_for("Maria").unwrap();
        assert_eq!(maria_edges.len(), 2, "maria->pedro + a espelhada joao<-maria");
        let maria_social = svc.get_person("Maria").unwrap().unwrap();
        let maria_catalog = catalog.find_by_alias("Maria").unwrap();
        assert_eq!(maria_social.catalog_id.as_deref(), Some(maria_catalog.id.as_str()), "identidade única (item 2)");

        let hops = svc.relationship_path("João", "Pedro", 3).unwrap().expect("2 saltos via maria");
        assert_eq!(hops.len(), 2);
    }

    /// A frase do usuário já entra redigida como `source`, mas o TEXTO do fato (saída do modelo) ia cru
    /// para o `catalog.json`: um e-mail ou telefone dito no chat ficava em claro no catálogo e no export.
    #[test]
    fn ingest_extraction_redacts_contact_data_in_a_fact_before_it_reaches_the_catalog() {
        let dir = TempDir::new();
        let catalog = open_catalog(&dir);
        let fact = PersonOp::AddFact { target: PersonRef { name: Some("Dani".to_string()), ..Default::default() }, fact: "email dela é dani@example.com".to_string(), confidence: Some(0.9) };
        let extracted = ExtractionResult { ops: vec![extracted_person("Dani", Some("esposa")), fact], relationships: vec![] };

        ingest_extraction(&catalog, None, true, &extracted, "o email da minha esposa é dani@example.com", 1, 100, "Mika");

        let dani = catalog.find_by_alias("Dani").expect("dani catalogada");
        let facts: Vec<&str> = dani.facts.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(facts, vec!["email dela é [EMAIL]"]);
    }

    /// Item 2: nem o usuário (auto-apresentado NESTA mensagem) nem Mika viram registro de
    /// estranho, e nenhuma aresta com um dos dois como ponta é gravada.
    #[test]
    fn ingest_extraction_excludes_the_user_and_mika_and_any_edge_touching_them() {
        let dir = TempDir::new();
        let catalog = open_catalog(&dir);
        let mem = MemoryStore::open_in_memory().unwrap();
        let extracted = ExtractionResult {
            ops: vec![extracted_person("Antônio", None), extracted_person("Mika", None), extracted_person("Bob", None)],
            relationships: vec![edge("Mika", "Bob", "conversa com")],
        };
        let text = "Meu nome é Antônio, meu amigo é o Bob.";

        ingest_extraction(&catalog, Some(&mem), true, &extracted, text, 1, 100, "Mika");

        assert!(catalog.find_by_alias("Antônio").is_none(), "usuário não pode virar estranho catalogado");
        assert!(catalog.find_by_alias("Mika").is_none(), "Mika não pode se catalogar a si mesma");
        assert!(catalog.find_by_alias("Bob").is_some(), "terceiro legítimo continua sendo catalogado");

        let svc = SocialMemoryService::new(&mem);
        assert!(svc.relationships_for("Bob").unwrap().is_empty(), "aresta que toca um nome excluído não pode ser gravada");
    }

    #[test]
    fn ingest_extraction_writes_nothing_when_memory_recording_is_off() {
        let dir = TempDir::new();
        let catalog = open_catalog(&dir);
        let mem = MemoryStore::open_in_memory().unwrap();
        let extracted = ExtractionResult { ops: vec![extracted_person("Dani", None)], relationships: vec![edge("Dani", "Bob", "colega")] };

        ingest_extraction(&catalog, Some(&mem), false, &extracted, "minha esposa Dani gosta de sorvete", 1, 100, "Mika");

        assert!(catalog.find_by_alias("Dani").is_none(), "com a memória desligada a pessoa não entra no catálogo");
        assert!(SocialMemoryService::new(&mem).get_graph().unwrap().people.is_empty());
    }

    #[test]
    fn ingest_extraction_without_a_memory_store_still_upserts_the_catalog() {
        let dir = TempDir::new();
        let catalog = open_catalog(&dir);
        let extracted = ExtractionResult { ops: vec![extracted_person("Ada", None)], relationships: vec![] };

        ingest_extraction(&catalog, None, true, &extracted, "Ada apareceu", 1, 100, "Mika");

        assert!(catalog.find_by_alias("Ada").is_some(), "degrada sem grafo social, mas o catálogo continua funcionando");
    }

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            // Contador, não relógio: no Windows testes paralelos liam o mesmo nanossegundo.
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-person-catalog-ingest-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
