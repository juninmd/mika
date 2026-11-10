//! Grafo de memória social — porte de `apps/desktop/src/main/services/memory/social/`
//! (`SocialMemoryService` + `person-graph`/`relationship-graph`/`fact-store`/`social-store`).
//!
//! ## Onde o TypeScript persiste isto de verdade (lido, não assumido)
//! `initSocialMemory` (`services/brain/services/brain-social-init.ts`) grava no MESMO banco que a
//! memória episódica: `createMemoryStore({ backend, db: socialStoreDb })`, e `socialStoreDb` é
//! `db.getDatabase('memory.db', 'plugins/mika.brain')` — o arquivo que `bootstrap/brain.ts` também
//! abre para `db` (não um `socialDb` separado, salvo migração legada). Com o backend padrão
//! (`DEFAULT_MEMORY_BACKEND = 'embedded'`, `store-factory.ts`), isto é `EmbeddedMemoryStore` — a
//! MESMA tabela `memory_entries(key, field, value, score, expires_at)` que
//! [`crate::modules::memory::MemoryStore`] já porta byte a byte (ver o cabeçalho de `mod.rs`, um
//! nível acima). **Não é SQLite dedicado do grafo social** — é uma fatia por prefixo de chave
//! (`mika:social:<tabela>:<id>`, `social-redis-index.ts`) do MESMO banco chave/valor que `episodic`
//! usa, dentro do MESMO arquivo físico. Por isso este módulo não abre conexão nenhuma: recebe
//! `&MemoryStore` já aberto (ver [`crate::modules::memory::social_reader`]).
//!
//! ## O que dispacha de verdade nesta rodada
//! `add_fact`/`delete_fact`/`add_relationship`/`delete_relationship` (as 4 ações que
//! `features::person_catalog::tool.rs` e `brain::tools::memory_console.rs` degradavam nomeando este
//! grafo como causa), `forget_person` (varre pessoa + relações + fatos + eventos — usado pelo
//! `delete` de `person_catalog::tool.rs`) e `get_graph` (o `graph` de `memory_console::snapshot`).
//!
//! ## O que ficou de fora, nomeado
//! `addEvent`/`updateAffinity`/`buildContext` (`social-context.ts`) não têm chamador degradando
//! nomeado neste grafo nesta rodada — nenhuma tool pede porte deles hoje. `get_graph` ainda LÊ o
//! perfil (`social_user_profile`) para não fingir `null` quando existe dado real.
//!
//! `setUserProfile`/`forgetUserProfile`/`setPersonalityProfile` — a premissa de que estes três
//! "não têm chamador degradando nomeado" caiu nesta rodada: `features::user::tool` (`set`/
//! `forget`/`set_personality`) já degradava nomeando exatamente estes três métodos ausentes. Ver
//! [`user_profile`] para a implementação e a semântica de merge medida linha a linha no TS.

mod fact_rules;
mod fact_store;
mod person_graph;
mod query;
mod relationship_graph;
mod rpc;
mod rules;
mod store;
pub mod types;
mod user_profile;

use serde::Serialize;

use crate::modules::memory::MemoryStore;

pub use fact_store::AddFactInput;
pub use person_graph::UpsertPersonInput;
pub use rules::{resolve_self_subject, USER_SUBJECT};
pub use relationship_graph::PathHop;
/// = `mika:plugin-rpc` de `mika.brain` (ver `rpc.rs`). Falta a linha em
/// `modules::register_config_bound_tools` (fora deste diretório, reportada nesta rodada).
pub(crate) use rpc::register as register_rpc;
use types::{LearnedFact, PersonRecord, RelationshipEdge, UserProfile, SOCIAL_TABLE_FACTS, SOCIAL_TABLE_RELATIONSHIPS, SOCIAL_TABLE_USER_PROFILE};
pub use user_profile::{ForgetOutcome, ForgetUserProfileInput, PersonalityInput, SetUserProfileInput};

/// Entrada de `add_relationship` — espelha `SocialMemoryService.addRelationship`.
pub struct AddRelationshipInput {
    pub name: String,
    pub label: String,
    pub confidence: Option<f64>,
}

/// Entrada de [`SocialMemoryService::add_person_to_person`] — a metade "pessoa-pessoa" do pacote
/// "people catalog + relationship graph" (ex. "João trabalha com Maria"), companheira de
/// [`AddRelationshipInput`] (usuário-pessoa).
pub struct AddPersonToPersonInput {
    pub from_name: String,
    pub to_name: String,
    pub label: String,
    pub confidence: Option<f64>,
}

/// = o `graph` de `context-console.ts::snapshot` / `social-context.ts::getGraph`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialGraph {
    pub profile: Option<serde_json::Value>,
    pub people: Vec<PersonRecord>,
    pub relationships: Vec<RelationshipEdge>,
    pub facts: Vec<LearnedFact>,
}

/// Fachada fina sobre um [`MemoryStore`] já aberto — mesmo papel que `RedisSocialStore` cumpre no
/// TypeScript, mas sem estado próprio (nenhum campo além da referência), então construir uma
/// instância por chamada é barato e não exige `Clone`/`Arc` no chamador.
pub struct SocialMemoryService<'a> {
    mem: &'a MemoryStore,
}

impl<'a> SocialMemoryService<'a> {
    pub fn new(mem: &'a MemoryStore) -> Self {
        Self { mem }
    }

    /// = `SocialMemoryService.addFact`. `Ok(None)` quando `memory.enabled=false` (mesmo efeito do
    /// `MemoryRecordingDisabledError` do TS: nada foi gravado, o chamador nunca finge sucesso).
    pub fn add_fact(&self, recording_allowed: bool, input: AddFactInput, now_ms: i64) -> Result<Option<LearnedFact>, String> {
        if !recording_allowed {
            return Ok(None);
        }
        fact_store::add_fact(self.mem, input, now_ms).map(Some)
    }

    /// O `MemoryStore` por baixo — para quem precisa varrer outras camadas (episódica) no mesmo
    /// passo, como o "esquecer pessoa" do painel de memória.
    pub fn memory(&self) -> &'a MemoryStore {
        self.mem
    }

    /// = `SocialMemoryService.deleteFact`, e traz de volta o que este fato aposentou: apagar o fato
    /// novo (errado) deixava o antigo `active: false` com `superseded_by` apontando para um id que
    /// não existe mais — a Mika "esquecia" os dois.
    pub fn delete_fact(&self, id: &str) -> Result<bool, String> {
        let deleted = store::delete_if_exists(self.mem, SOCIAL_TABLE_FACTS, id)?;
        if deleted {
            let facts: Vec<LearnedFact> = store::get_all(self.mem, SOCIAL_TABLE_FACTS)?;
            for old in facts.into_iter().filter(|f| f.superseded_by.as_deref() == Some(id)) {
                let revived = LearnedFact { active: Some(true), superseded_by: None, ..old };
                store::upsert(self.mem, SOCIAL_TABLE_FACTS, &revived.id, revived.updated_at as f64, &revived)?;
            }
        }
        Ok(deleted)
    }

    /// Edição de um fato pelo usuário (painel de memória). `Ok(None)`: id inexistente. Passa pela
    /// mesma redação de segredos de [`Self::add_fact`] e gera id novo para o texto novo — ver
    /// [`fact_store::update_fact`]. Respeita `memory.enabled` como toda escrita.
    pub fn update_fact(&self, recording_allowed: bool, id: &str, text: &str, confidence: Option<f64>, now_ms: i64) -> Result<Option<LearnedFact>, String> {
        if !recording_allowed {
            return Err("memory recording is disabled (memory.enabled=false)".to_string());
        }
        fact_store::update_fact(self.mem, id, text, confidence, now_ms)
    }

    /// = `SocialMemoryService.addRelationship`: primeiro funde/cria a pessoa-alvo (tipo inferido do
    /// rótulo do vínculo, `inferPersonType`), depois grava a aresta — mesma composição do TS.
    pub fn add_relationship(&self, recording_allowed: bool, input: AddRelationshipInput, now_ms: i64) -> Result<Option<RelationshipEdge>, String> {
        if !recording_allowed {
            return Ok(None);
        }
        let kind = rules::infer_person_type(Some(&input.label));
        let person = person_graph::upsert_person(
            self.mem,
            UpsertPersonInput { name: input.name, kind: Some(kind), ..Default::default() },
            now_ms,
        )?;
        relationship_graph::add_relationship(self.mem, &person.id, &input.label, input.confidence, now_ms).map(Some)
    }

    /// = `SocialMemoryService.deleteRelationship`.
    pub fn delete_relationship(&self, id: &str) -> Result<bool, String> {
        store::delete_if_exists(self.mem, SOCIAL_TABLE_RELATIONSHIPS, id)
    }

    /// Pessoa-pessoa (ex. "João trabalha com Maria" / correção "não, a Maria é minha prima"):
    /// funde/cria as DUAS pessoas envolvidas (mesma composição de [`Self::add_relationship`]) e
    /// grava a aresta com os dois ids — supersedendo, não empilhando, uma aresta ativa anterior
    /// para o MESMO par (ver [`relationship_graph::supersede_pair`]).
    pub fn add_person_to_person(&self, recording_allowed: bool, input: AddPersonToPersonInput, now_ms: i64) -> Result<Option<RelationshipEdge>, String> {
        if !recording_allowed {
            return Ok(None);
        }
        let from = person_graph::upsert_person(self.mem, UpsertPersonInput { name: input.from_name, ..Default::default() }, now_ms)?;
        let to = person_graph::upsert_person(self.mem, UpsertPersonInput { name: input.to_name, ..Default::default() }, now_ms)?;
        relationship_graph::upsert_person_to_person(self.mem, &from.id, &to.id, &input.label, input.confidence, now_ms).map(Some)
    }

    /// = a ação `relationships {name}` da tool (item 4 do pacote): toda aresta ativa que toca
    /// `identifier` (pessoa OU usuário), nos dois sentidos.
    pub fn relationships_for(&self, identifier: &str) -> Result<Vec<RelationshipEdge>, String> {
        relationship_graph::relationships_for(self.mem, identifier)
    }

    /// = a ação `path {from, to}` da tool: BFS até profundidade 3 sobre o grafo social inteiro.
    pub fn relationship_path(&self, from: &str, to: &str, max_depth: usize) -> Result<Option<Vec<PathHop>>, String> {
        relationship_graph::path(self.mem, from, to, max_depth)
    }

    /// Item 2 do pacote "people catalog + relationship graph" — identidade única: liga (funde) o
    /// registro do grafo social desta pessoa ao `pc_*` do `person_catalog`, para que uma escrita
    /// social feita DEPOIS por esse mesmo nome (`add_fact`/`add_relationship`/
    /// `add_person_to_person`) resolva à MESMA pessoa que o catálogo já conhece, em vez de um
    /// segundo registro sem relação nenhuma (o defeito nomeado no cabeçalho deste módulo). Nunca
    /// gateada por `memory.enabled`: é a mesma classe de escrita "administrativa" que
    /// `person_graph::upsert_person` já faz sem esse gate em todo o resto deste arquivo.
    pub fn link_catalog_person(&self, name: &str, catalog_id: &str, now_ms: i64) -> Result<PersonRecord, String> {
        person_graph::upsert_person(self.mem, UpsertPersonInput { name: name.to_string(), catalog_id: Some(catalog_id.to_string()), ..Default::default() }, now_ms)
    }

    /// = `person-graph.ts::getPerson`.
    pub fn get_person(&self, identifier: &str) -> Result<Option<PersonRecord>, String> {
        person_graph::get_person(self.mem, identifier)
    }

    /// = `person-graph.ts::forgetPerson`. Usado por `person_catalog::tool.rs::handle_delete` para a
    /// metade "grafo social" de `forgetPersonEverywhere` (a outra metade, episódica, chama
    /// `crate::modules::memory::episodic::EpisodicMemoryService::forget_mentions` direto — ver o
    /// cabeçalho daquele arquivo).
    pub fn forget_person(&self, identifier: &str) -> Result<bool, String> {
        person_graph::forget_person(self.mem, identifier)
    }

    /// Consulta o grafo de fatos além de pessoas — implementação e testes em [`query`] (extraído
    /// para não estourar o limite de 300 linhas deste arquivo).
    pub fn query_facts(&self, subject: Option<&str>, predicate: Option<&str>, limit: usize) -> Result<Vec<LearnedFact>, String> {
        query::query_facts(self.mem, subject, predicate, limit)
    }

    /// = `social-context.ts::getGraph`.
    pub fn get_graph(&self) -> Result<SocialGraph, String> {
        Ok(SocialGraph {
            profile: store::get_by_id::<serde_json::Value>(self.mem, SOCIAL_TABLE_USER_PROFILE, "user")?,
            people: store::get_all(self.mem, types::SOCIAL_TABLE_PEOPLE)?,
            relationships: store::get_all(self.mem, SOCIAL_TABLE_RELATIONSHIPS)?,
            facts: store::get_all(self.mem, SOCIAL_TABLE_FACTS)?,
        })
    }

    /// = `social-context.ts::clearAll`: varre as MESMAS 5 tabelas de `SOCIAL_TABLES` (mesma ordem),
    /// apaga registro a registro (`store::delete`, que também limpa os dois índices) e soma quantos
    /// saíram — chamado pela primeira vez nesta rodada por `features::privacy::clear` (ver o
    /// cabeçalho do módulo: "sem chamador degradando nomeado" deixou de ser verdade agora).
    pub fn clear_all(&self) -> Result<usize, String> {
        let mut removed = 0usize;
        for table in [
            types::SOCIAL_TABLE_USER_PROFILE,
            types::SOCIAL_TABLE_PEOPLE,
            types::SOCIAL_TABLE_RELATIONSHIPS,
            types::SOCIAL_TABLE_FACTS,
            types::SOCIAL_TABLE_EVENTS,
        ] {
            let records: Vec<serde_json::Value> = store::get_all(self.mem, table)?;
            for record in &records {
                let id = match record.get("id") {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    // Registro sem `id` utilizável não é apagável por `store::delete` — e por isso
                    // NÃO entra na conta. Somar `records.len()` no fim contaria este aqui como
                    // removido, e a contagem é o que `clear_history` mostra ao usuário: seria dizer
                    // "apaguei" sobre um registro que continua no disco.
                    _ => continue,
                };
                store::delete(self.mem, table, &id)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// = `SocialMemoryService.getUserProfile`. Nunca gateado por `memory.enabled` — o gate protege
    /// uma ESCRITA, ler o que já existe é sempre permitido (mesma política de [`Self::get_graph`]).
    pub fn get_user_profile(&self) -> Result<Option<UserProfile>, String> {
        user_profile::get_user_profile(self.mem)
    }

    /// = `SocialMemoryService.setUserProfile`. Semântica de merge documentada no cabeçalho de
    /// [`user_profile`] — nunca zera um campo que o `input` não menciona.
    pub fn set_user_profile(&self, recording_allowed: bool, input: SetUserProfileInput, now_ms: i64) -> Result<Option<UserProfile>, String> {
        user_profile::set_user_profile(self.mem, recording_allowed, input, now_ms)
    }

    /// = `SocialMemoryService.forgetUserProfile`. Ver [`ForgetOutcome`] para por que o retorno não
    /// é só `Option<UserProfile>`.
    pub fn forget_user_profile(&self, recording_allowed: bool, input: ForgetUserProfileInput, now_ms: i64) -> Result<ForgetOutcome, String> {
        user_profile::forget_user_profile(self.mem, recording_allowed, input, now_ms)
    }

    /// = `SocialMemoryService.setPersonalityProfile`.
    pub fn set_personality_profile(&self, recording_allowed: bool, input: PersonalityInput, now_ms: i64) -> Result<Option<UserProfile>, String> {
        user_profile::set_personality_profile(self.mem, recording_allowed, input, now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::types::PersonType;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn add_fact_blocked_by_memory_enabled_false_persists_nothing() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let out = svc.add_fact(false, AddFactInput { fact: "nao deveria gravar".into(), ..Default::default() }, 1).unwrap();
        assert_eq!(out, None);
        assert!(svc.get_graph().unwrap().facts.is_empty());
    }

    #[test]
    fn add_fact_then_delete_fact_round_trips_through_the_service() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let fact = svc.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap().unwrap();

        assert!(svc.delete_fact(&fact.id).unwrap());
        assert!(!svc.delete_fact(&fact.id).unwrap(), "segunda chamada não finge sucesso");
    }

    #[test]
    fn add_relationship_blocked_by_memory_enabled_false_creates_no_person_either() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let out = svc.add_relationship(false, AddRelationshipInput { name: "Ada".into(), label: "amiga".into(), confidence: None }, 1).unwrap();
        assert_eq!(out, None);
        assert_eq!(svc.get_person("Ada").unwrap(), None, "pessoa não pode ter sido criada por uma escrita bloqueada");
    }

    /// Item 1 do pacote "people catalog + relationship graph", via a fachada de serviço: "João
    /// trabalha com Maria" cria as duas pessoas E a aresta com os dois ids — gravada duas vezes em
    /// disco (direta joao->maria, espelhada maria->joao, ver `relationship_graph::
    /// upsert_person_to_person`), mas [`SocialMemoryService::relationships_for`] devolve exatamente
    /// UMA linha por relação por pessoa (a espelhada, do lado da Maria) — contar as duas contaria a
    /// MESMA relação em dobro, o bug nomeado no exemplo de aceitação da tarefa (item 4).
    #[test]
    fn add_person_to_person_creates_both_people_and_the_two_ended_edge() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let input = AddPersonToPersonInput { from_name: "João".into(), to_name: "Maria".into(), label: "trabalha com".into(), confidence: None };
        let edge = svc.add_person_to_person(true, input, 1).unwrap().unwrap();

        let joao = svc.get_person("João").unwrap().unwrap();
        let maria = svc.get_person("Maria").unwrap().unwrap();
        assert_eq!(edge.person_id, joao.id);
        assert_eq!(edge.to_person_id, Some(maria.id.clone()));

        assert_eq!(svc.relationships_for("Maria").unwrap().len(), 1, "uma relação só pode aparecer uma vez, não uma por ponta gravada");
    }

    #[test]
    fn add_person_to_person_blocked_by_memory_enabled_false_creates_no_one() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let input = AddPersonToPersonInput { from_name: "João".into(), to_name: "Maria".into(), label: "trabalha com".into(), confidence: None };
        assert_eq!(svc.add_person_to_person(false, input, 1).unwrap(), None);
        assert_eq!(svc.get_person("João").unwrap(), None);
    }

    /// "como o João conhece a Maria?" através da fachada de serviço.
    #[test]
    fn relationship_path_answers_how_two_people_know_each_other() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_person_to_person(true, AddPersonToPersonInput { from_name: "João".into(), to_name: "Maria".into(), label: "trabalha com".into(), confidence: None }, 1).unwrap();

        let hops = svc.relationship_path("João", "Maria", 3).unwrap().expect("caminho direto existe");
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].label, "trabalha com");
    }

    #[test]
    fn add_relationship_creates_the_person_and_the_edge_together() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        let edge = svc.add_relationship(true, AddRelationshipInput { name: "Ada Lovelace".into(), label: "amiga".into(), confidence: None }, 1).unwrap().unwrap();

        let person = svc.get_person("Ada Lovelace").unwrap().unwrap();
        assert_eq!(person.kind, PersonType::Friend, "tipo inferido do rótulo do vínculo");
        assert_eq!(edge.person_id, person.id);

        assert!(svc.delete_relationship(&edge.id).unwrap());
    }

    #[test]
    fn clear_all_wipes_facts_relationships_and_the_person_created_for_them() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap();
        // "amiga" é seu próprio inverso em `rel_inverse()` (relationship_graph.rs) — `add_relationship`
        // grava a aresta direta E a implícita, 2 registros em `SOCIAL_TABLE_RELATIONSHIPS`, não 1.
        svc.add_relationship(true, AddRelationshipInput { name: "Ada Lovelace".into(), label: "amiga".into(), confidence: None }, 1).unwrap();
        mem.set("mika:social:social_user_profile:user", r#"{"id":"user","languages":[],"facts":[],"updatedAt":1}"#, None).unwrap();

        let removed = svc.clear_all().unwrap();
        assert_eq!(removed, 5, "1 fato + 1 pessoa + 2 vínculos (direto + inverso implícito) + 1 perfil");

        let graph = svc.get_graph().unwrap();
        assert!(graph.facts.is_empty());
        assert!(graph.people.is_empty());
        assert!(graph.relationships.is_empty());
        assert!(graph.profile.is_none(), "perfil social também tem de sair no wipe completo");
    }

    /// NEGATIVO — depois do wipe, uma segunda `clear_all` não encontra mais nada pra apagar (idempotente).
    #[test]
    fn clear_all_twice_in_a_row_is_a_harmless_no_op_the_second_time() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap();

        assert_eq!(svc.clear_all().unwrap(), 1);
        assert_eq!(svc.clear_all().unwrap(), 0);
    }

    #[test]
    fn get_graph_reads_a_previously_written_profile_without_ever_writing_one() {
        let mem = store();
        mem.set("mika:social:social_user_profile:user", r#"{"id":"user","languages":[],"facts":[],"updatedAt":1}"#, None).unwrap();
        let svc = SocialMemoryService::new(&mem);

        let graph = svc.get_graph().unwrap();
        assert!(graph.profile.is_some(), "perfil gravado por outro processo (ou pelo TS) tem de aparecer aqui");
    }

    /// Prova de fiação: `SocialMemoryService::set_user_profile` grava na MESMA tabela que
    /// `get_graph()` já lê — as duas rotas de leitura do perfil (tipada e crua) enxergam a mesma
    /// escrita. A semântica do merge em si é testada em `user_profile.rs`, não aqui.
    #[test]
    fn set_user_profile_through_the_service_is_visible_to_get_graph_and_get_user_profile() {
        let mem = store();
        let svc = SocialMemoryService::new(&mem);
        svc.set_user_profile(true, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();

        assert_eq!(svc.get_user_profile().unwrap().unwrap().preferred_name.as_deref(), Some("Ada"));
        assert_eq!(svc.get_graph().unwrap().profile.unwrap()["preferredName"], serde_json::json!("Ada"));
    }

    #[test]
    fn deleting_the_fact_that_superseded_another_brings_the_original_back() {
        let kv = store();
        let svc = SocialMemoryService::new(&kv);
        let old = svc.add_fact(true, AddFactInput { fact: "fuma cigarro".into(), ..Default::default() }, 1).unwrap().unwrap();
        let newer = svc.add_fact(true, AddFactInput { fact: "parou de fumar cigarro".into(), ..Default::default() }, 2).unwrap().unwrap();

        assert!(svc.delete_fact(&newer.id).unwrap());
        let facts = svc.query_facts(None, None, 10).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].id, old.id);
        assert_eq!(facts[0].active, Some(true));
        assert_eq!(facts[0].superseded_by, None, "sem ponteiro pendurado");
    }

    #[test]
    fn forgetting_a_person_also_drops_edges_pointing_to_her_and_facts_filed_under_a_nickname() {
        let kv = store();
        let svc = SocialMemoryService::new(&kv);
        svc.add_person_to_person(true, AddPersonToPersonInput { from_name: "João".into(), to_name: "Maria".into(), label: "colega".into(), confidence: None }, 1).unwrap();
        svc.add_fact(true, AddFactInput { subject: Some("Maria".into()), fact: "gosta de café".into(), ..Default::default() }, 2).unwrap();
        let maria = svc.get_person("Maria").unwrap().unwrap();
        // apelido registrado: fatos arquivados sob ele também são dela
        person_graph::upsert_person(&kv, UpsertPersonInput { name: "Maria".into(), aliases: Some(vec!["Mari".into()]), ..Default::default() }, 3).unwrap();
        svc.add_fact(true, AddFactInput { subject: Some("Mari".into()), fact: "odeia coentro".into(), ..Default::default() }, 4).unwrap();

        assert!(svc.forget_person("Maria").unwrap());
        assert!(svc.get_person("Maria").unwrap().is_none());
        assert!(svc.relationships_for("João").unwrap().is_empty(), "a aresta João→Maria não pode sobrar");
        assert!(svc.query_facts(None, None, 10).unwrap().iter().all(|f| f.subject != "Mari" && f.subject != "Maria"), "{:?} / {}", svc.query_facts(None, None, 10).unwrap(), maria.id);
    }
}
