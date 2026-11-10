//! Porte de `apps/desktop/src/main/services/memory/social/relationship-graph.ts`, estendido pelo
//! pacote "people catalog + relationship graph": aresta pessoa-pessoa com os DOIS ids explícitos
//! (`upsert_person_to_person`, em vez de `add_relationship` reusar `person_id` para as duas pontas),
//! supersede-por-par com histórico (`valid_to`, `supersede_pair`) e travessia em grafo
//! (`relationships_for`/`path`, BFS até profundidade 3) para as ações de leitura `relationships`/
//! `path` da tool `person_catalog`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use serde::Serialize;

use crate::modules::memory::MemoryStore;

use super::person_graph;
use super::rules::normalize_text;
use super::store;
use super::types::{RelationshipDirection, RelationshipEdge, SOCIAL_TABLE_RELATIONSHIPS};

/// Pseudo-nó usado pela travessia (`path`/`relationships_for`) para representar "o usuário" — o
/// grafo social nunca dá um id de pessoa a ele (`UserToPerson` deixa `to_person_id` implícito).
const USER_NODE: &str = "user";
/// Termos que a tool/LLM podem usar para se referir ao usuário num `identifier` de `path`/
/// `relationships` — mesmo vocabulário que `rules::is_valid_person_name` já recusa como nome válido.
const USER_ALIASES: [&str; 4] = ["user", "eu", "voce", "usuario"];

/// = `REL_INVERSE`.
fn rel_inverse() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("namorada", "namorado"),
            ("namorado", "namorada"),
            ("esposa", "esposo"),
            ("esposo", "esposa"),
            ("wife", "husband"),
            ("husband", "wife"),
            ("girlfriend", "boyfriend"),
            ("boyfriend", "girlfriend"),
            ("mae", "filho"),
            ("pai", "filho"),
            ("mother", "child"),
            ("father", "child"),
            ("filho", "pai"),
            ("filha", "mae"),
            ("chefe", "funcionario"),
            ("boss", "employee"),
            ("funcionario", "chefe"),
            ("employee", "boss"),
            ("amigo", "amigo"),
            ("amiga", "amiga"),
            ("friend", "friend"),
            ("irmao", "irmao"),
            ("irma", "irma"),
            ("sibling", "sibling"),
        ])
    })
}

/// = `addRelationship`: grava o vínculo pedido e, quando existe inverso conhecido (ex. namorada ->
/// namorado), grava também a aresta `person_to_person` implícita — sem isso o grafo só sabe olhar
/// numa direção.
pub fn add_relationship(mem: &MemoryStore, person_id: &str, label: &str, confidence: Option<f64>, now: i64) -> Result<RelationshipEdge, String> {
    let conf = confidence.unwrap_or(0.85);
    let edge = RelationshipEdge {
        id: format!("{person_id}:{}", normalize_text(label)),
        person_id: person_id.to_string(),
        to_person_id: None,
        label: label.trim().to_string(),
        direction: RelationshipDirection::UserToPerson,
        confidence: conf,
        updated_at: now,
        valid_to: None,
    };
    store::upsert(mem, SOCIAL_TABLE_RELATIONSHIPS, &edge.id, now as f64, &edge)?;

    if let Some(inverse_label) = rel_inverse().get(normalize_text(label).as_str()) {
        let inverse = RelationshipEdge {
            id: format!("{person_id}:{}:inverse", normalize_text(inverse_label)),
            person_id: person_id.to_string(),
            to_person_id: None,
            label: inverse_label.to_string(),
            direction: RelationshipDirection::PersonToPerson,
            confidence: conf,
            updated_at: now,
            valid_to: None,
        };
        store::upsert(mem, SOCIAL_TABLE_RELATIONSHIPS, &inverse.id, now as f64, &inverse)?;
    }

    Ok(edge)
}

/// Rótulo espelhado para a aresta `(to_id -> from_id)` que [`upsert_person_to_person`] também grava
/// — o inverso conhecido de [`rel_inverse`] quando existe (ex. "chefe" -> "funcionario"), senão o
/// MESMO rótulo ("trabalha com"/"mora com"/"joga com" já leem igual nos dois sentidos — "symmetric
/// where natural" do pacote da tarefa). Nunca inventa um inverso que o par não pediu.
fn mirrored_label(label: &str) -> String {
    rel_inverse().get(normalize_text(label).as_str()).map(|inv| inv.to_string()).unwrap_or_else(|| label.trim().to_string())
}

/// Contador monotônico para desempatar ids de aresta na mesma janela de milissegundo — sem ele,
/// duas correções para o mesmo par no mesmo `now` (granularidade de `lifecycle::now_ms()`) geram o
/// MESMO id, e `store::upsert` (substitui-por-id) sobrescreve o registro que `supersede_pair` acabou
/// de aposentar em vez de manter os dois como histórico.
fn next_edge_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Aresta ATIVA (`valid_to: None`) entre exatamente este par ordenado, se houver — usado por
/// [`supersede_pair`] para saber o que retirar.
fn active_edge_for_pair<'a>(edges: &'a [RelationshipEdge], from_id: &str, to_id: &str) -> Vec<&'a RelationshipEdge> {
    edges
        .iter()
        .filter(|e| e.valid_to.is_none() && e.direction == RelationshipDirection::PersonToPerson && e.person_id == from_id && e.to_person_id.as_deref() == Some(to_id))
        .collect()
}

/// Porte do item 5 da tarefa ("corrections... supersede old edge; keep history with `valid_to`"):
/// toda aresta ATIVA já gravada para este par ORDENADO (`from_id -> to_id`) vira histórico
/// (`valid_to = now`, o registro em si nunca é apagado) antes de [`upsert_person_to_person`] gravar
/// a nova verdade. Escopado ao PAR — não à pessoa inteira — porque uma pessoa pode legitimamente ter
/// várias arestas simultâneas para alvos DIFERENTES (ex. João é ao mesmo tempo colega de Maria E
/// vizinho de Pedro); só uma nova afirmação sobre o MESMO par é uma correção.
fn supersede_pair(mem: &MemoryStore, from_id: &str, to_id: &str, now: i64) -> Result<(), String> {
    let all: Vec<RelationshipEdge> = store::get_all(mem, SOCIAL_TABLE_RELATIONSHIPS)?;
    for edge in active_edge_for_pair(&all, from_id, to_id) {
        let retired = RelationshipEdge { valid_to: Some(now), ..edge.clone() };
        store::upsert(mem, SOCIAL_TABLE_RELATIONSHIPS, &retired.id, now as f64, &retired)?;
    }
    Ok(())
}

/// = `addPersonToPersonRelationship` do pacote "people catalog + relationship graph": grava a
/// aresta pessoa-pessoa com AMBOS os ids (o bug que `RelationshipEdge` reusava `person_id` para as
/// duas pontas — ver o cabeçalho de `types.rs`), supersedendo (não empilhando) qualquer aresta ativa
/// já gravada para o MESMO par ordenado, e espelha a aresta inversa (`to_id -> from_id`) quando o
/// rótulo tem um inverso natural — mesmo par direto+inverso que [`add_relationship`] já grava para
/// usuário-pessoa.
pub fn upsert_person_to_person(mem: &MemoryStore, from_id: &str, to_id: &str, label: &str, confidence: Option<f64>, now: i64) -> Result<RelationshipEdge, String> {
    let conf = confidence.unwrap_or(0.7);
    let label = label.trim().to_string();

    supersede_pair(mem, from_id, to_id, now)?;
    let edge = RelationshipEdge {
        id: format!("{from_id}:{to_id}:{now}:{}", next_edge_seq()),
        person_id: from_id.to_string(),
        to_person_id: Some(to_id.to_string()),
        label: label.clone(),
        direction: RelationshipDirection::PersonToPerson,
        confidence: conf,
        updated_at: now,
        valid_to: None,
    };
    store::upsert(mem, SOCIAL_TABLE_RELATIONSHIPS, &edge.id, now as f64, &edge)?;

    if from_id != to_id {
        supersede_pair(mem, to_id, from_id, now)?;
        let mirror = RelationshipEdge {
            id: format!("{to_id}:{from_id}:{now}:inverse:{}", next_edge_seq()),
            person_id: to_id.to_string(),
            to_person_id: Some(from_id.to_string()),
            label: mirrored_label(&label),
            direction: RelationshipDirection::PersonToPerson,
            confidence: conf,
            updated_at: now,
            valid_to: None,
        };
        store::upsert(mem, SOCIAL_TABLE_RELATIONSHIPS, &mirror.id, now as f64, &mirror)?;
    }

    Ok(edge)
}

/// = a ação `relationships {name}` da tool `person_catalog`: uma linha por relação ATIVA em que este
/// nó participa, pessoa OU usuário. `identifier` aceita um nome/apelido de pessoa OU um dos
/// [`USER_ALIASES`]. Filtra só por `person_id == node`, nunca também por `to_person_id == node`:
/// [`upsert_person_to_person`] já grava as DUAS pontas de cada relação pessoa-pessoa como arestas
/// separadas (uma com `person_id` de cada lado — a direta e a espelhada), então toda relação em que
/// este nó participa SEMPRE tem exatamente uma aresta com `person_id == node` (a dele próprio,
/// direta ou espelhada). Casar também por `to_person_id` contaria a MESMA relação duas vezes — bug
/// nomeado no exemplo de aceitação da tarefa (item 4): João/Maria/Pedro tem 2 relações tocando
/// Maria, não 4. Também exclui a aresta-marcador legada de [`add_relationship`] (`PersonToPerson`
/// com `to_person_id: None`, o "inverso implícito" tipo namorada/namorado) — ela não tem uma
/// segunda pessoa de verdade para reportar, mesmo tratamento que [`build_adjacency`] já dá a ela.
pub fn relationships_for(mem: &MemoryStore, identifier: &str) -> Result<Vec<RelationshipEdge>, String> {
    let Some(node) = resolve_node_id(mem, identifier)? else { return Ok(vec![]) };
    let all: Vec<RelationshipEdge> = store::get_all(mem, SOCIAL_TABLE_RELATIONSHIPS)?;
    Ok(all
        .into_iter()
        .filter(|e| e.valid_to.is_none() && e.person_id == node && !(e.direction == RelationshipDirection::PersonToPerson && e.to_person_id.is_none()))
        .collect())
}

/// Um salto do caminho devolvido por [`path`] — serializável direto na resposta da tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHop {
    pub from: String,
    pub label: String,
    pub to: String,
}

fn resolve_node_id(mem: &MemoryStore, identifier: &str) -> Result<Option<String>, String> {
    if USER_ALIASES.contains(&normalize_text(identifier).as_str()) {
        return Ok(Some(USER_NODE.to_string()));
    }
    Ok(person_graph::get_person(mem, identifier)?.map(|p| p.id))
}

/// Adjacência não-direcionada derivada das arestas ATIVAS: `UserToPerson` conecta o pseudo-nó
/// [`USER_NODE`] à pessoa; `PersonToPerson` conecta as duas pessoas quando `to_person_id` está
/// presente (uma aresta legada sem ele — grafada antes deste pacote — não tem segunda ponta para
/// conectar, e é ignorada aqui, não travada).
fn build_adjacency(edges: &[RelationshipEdge]) -> HashMap<String, Vec<(String, String)>> {
    let mut adjacency: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut connect = |a: String, b: String, label: String| {
        adjacency.entry(a).or_default().push((b, label));
    };
    for edge in edges.iter().filter(|e| e.valid_to.is_none()) {
        match (edge.direction, &edge.to_person_id) {
            (RelationshipDirection::UserToPerson, _) => {
                connect(USER_NODE.to_string(), edge.person_id.clone(), edge.label.clone());
                connect(edge.person_id.clone(), USER_NODE.to_string(), edge.label.clone());
            }
            (RelationshipDirection::PersonToPerson, Some(to_id)) => {
                connect(edge.person_id.clone(), to_id.clone(), edge.label.clone());
            }
            (RelationshipDirection::PersonToPerson, None) => {}
        }
    }
    adjacency
}

/// = a ação `path {from, to}` da tool `person_catalog` ("como o João conhece a Maria?"): BFS sobre
/// o grafo não-direcionado até `max_depth` (travado em 3 — a tarefa pede "max depth 3", e sem teto
/// uma travessia sobre um grafo social que cresce sem limite seria custo ilimitado por chamada).
/// `Ok(None)` quando `from`/`to` não resolve a um nó conhecido OU nenhum caminho existe dentro do
/// teto — a tool devolve os dois casos como "sem caminho encontrado", nunca inventa um.
pub fn path(mem: &MemoryStore, from: &str, to: &str, max_depth: usize) -> Result<Option<Vec<PathHop>>, String> {
    let depth = max_depth.clamp(1, 3);
    let (Some(from_id), Some(to_id)) = (resolve_node_id(mem, from)?, resolve_node_id(mem, to)?) else { return Ok(None) };
    if from_id == to_id {
        return Ok(Some(vec![]));
    }

    let all: Vec<RelationshipEdge> = store::get_all(mem, SOCIAL_TABLE_RELATIONSHIPS)?;
    let adjacency = build_adjacency(&all);

    let mut visited: HashSet<String> = HashSet::from([from_id.clone()]);
    let mut queue: VecDeque<(String, Vec<PathHop>)> = VecDeque::from([(from_id, Vec::new())]);
    while let Some((node, hops)) = queue.pop_front() {
        if hops.len() >= depth {
            continue;
        }
        for (neighbor, label) in adjacency.get(&node).cloned().unwrap_or_default() {
            let mut next_hops = hops.clone();
            next_hops.push(PathHop { from: node.clone(), label, to: neighbor.clone() });
            if neighbor == to_id {
                return Ok(Some(next_hops));
            }
            if visited.insert(neighbor.clone()) {
                queue.push_back((neighbor, next_hops));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_relationship_with_a_known_inverse_also_writes_the_implicit_edge() {
        let mem = MemoryStore::open_in_memory().unwrap();
        add_relationship(&mem, "ada-lovelace", "namorada", None, 1).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        assert_eq!(all.len(), 2, "aresta direta + inversa");
        assert!(all.iter().any(|e| e.label == "namorada" && e.direction == RelationshipDirection::UserToPerson));
        assert!(all.iter().any(|e| e.label == "namorado" && e.direction == RelationshipDirection::PersonToPerson));
    }

    #[test]
    fn add_relationship_without_a_known_inverse_writes_only_the_direct_edge() {
        let mem = MemoryStore::open_in_memory().unwrap();
        add_relationship(&mem, "ada-lovelace", "mentora", None, 1).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn add_relationship_defaults_confidence_to_0_85() {
        let mem = MemoryStore::open_in_memory().unwrap();
        let edge = add_relationship(&mem, "ada-lovelace", "amiga", None, 1).unwrap();
        assert_eq!(edge.confidence, 0.85);
    }

    // --- upsert_person_to_person -------------------------------------------------------------

    /// O bug nomeado no pacote da tarefa: uma aresta pessoa-pessoa precisa das DUAS pontas, não de
    /// `person_id` reusado — sem isso "João trabalha com Maria" não tem como devolver quem é Maria.
    #[test]
    fn upsert_person_to_person_stores_both_ends_and_mirrors_the_label() {
        let mem = MemoryStore::open_in_memory().unwrap();
        let edge = upsert_person_to_person(&mem, "joao", "maria", "trabalha com", None, 1).unwrap();
        assert_eq!(edge.person_id, "joao");
        assert_eq!(edge.to_person_id.as_deref(), Some("maria"));
        assert_eq!(edge.direction, RelationshipDirection::PersonToPerson);

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        assert_eq!(all.len(), 2, "aresta direta + espelhada");
        let mirror = all.iter().find(|e| e.person_id == "maria").expect("aresta espelhada maria->joao");
        assert_eq!(mirror.to_person_id.as_deref(), Some("joao"));
        assert_eq!(mirror.label, "trabalha com", "sem inverso conhecido, o rótulo espelha igual");
    }

    #[test]
    fn upsert_person_to_person_mirrors_a_known_inverse_label() {
        let mem = MemoryStore::open_in_memory().unwrap();
        upsert_person_to_person(&mem, "pedro", "maria", "esposo", None, 1).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        let mirror = all.iter().find(|e| e.person_id == "maria").unwrap();
        assert_eq!(mirror.label, "esposa", "inverso de 'esposo' é 'esposa', não o mesmo rótulo");
    }

    /// Item 5 ("corrections"): reafirmar o par com um rótulo novo aposenta (não empilha) a aresta
    /// anterior — o registro antigo continua legível (`valid_to` marcado), nunca é apagado.
    #[test]
    fn upsert_person_to_person_supersedes_the_previous_edge_for_the_same_pair() {
        let mem = MemoryStore::open_in_memory().unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "esposa", None, 1).unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "prima", None, 2).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        let joao_to_maria: Vec<&RelationshipEdge> = all.iter().filter(|e| e.person_id == "joao" && e.to_person_id.as_deref() == Some("maria")).collect();
        assert_eq!(joao_to_maria.len(), 2, "aresta antiga fica retida, não é apagada");
        let active: Vec<&&RelationshipEdge> = joao_to_maria.iter().filter(|e| e.valid_to.is_none()).collect();
        assert_eq!(active.len(), 1, "só UMA aresta pode estar ativa para o mesmo par");
        assert_eq!(active[0].label, "prima");
        let retired = joao_to_maria.iter().find(|e| e.label == "esposa").unwrap();
        assert_eq!(retired.valid_to, Some(2));
    }

    /// Corrida: duas correções para o MESMO par no MESMO `now` (granularidade de milissegundo de
    /// `lifecycle::now_ms()`) não podem colidir de id — sem o contador de `next_edge_seq`, o segundo
    /// `upsert` sobrescreve (`store::upsert` é substitui-por-id) o registro que `supersede_pair`
    /// acabou de aposentar, apagando o histórico que o item 5 promete manter.
    #[test]
    fn upsert_person_to_person_keeps_history_when_same_millisecond_supersedes_twice() {
        let mem = MemoryStore::open_in_memory().unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "colega", None, 5).unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "prima", None, 5).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        let joao_to_maria: Vec<&RelationshipEdge> = all.iter().filter(|e| e.person_id == "joao" && e.to_person_id.as_deref() == Some("maria")).collect();
        assert_eq!(joao_to_maria.len(), 2, "mesmo timestamp não pode fundir as duas arestas em uma só");
        assert!(joao_to_maria.iter().any(|e| e.label == "colega" && e.valid_to == Some(5)), "a antiga tem de sobreviver aposentada");
        assert!(joao_to_maria.iter().any(|e| e.label == "prima" && e.valid_to.is_none()), "a nova fica ativa");
    }

    /// NEGATIVO — um par DIFERENTE não é afetado por um supersede em outro par (escopo é o par, não
    /// a pessoa inteira: ver o comentário de `supersede_pair`).
    #[test]
    fn upsert_person_to_person_does_not_supersede_a_different_pair() {
        let mem = MemoryStore::open_in_memory().unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "colega", None, 1).unwrap();
        upsert_person_to_person(&mem, "joao", "pedro", "vizinho", None, 2).unwrap();

        let all: Vec<RelationshipEdge> = store::get_all(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap();
        let joao_to_maria = all.iter().find(|e| e.person_id == "joao" && e.to_person_id.as_deref() == Some("maria")).unwrap();
        assert!(joao_to_maria.valid_to.is_none(), "par não relacionado não pode ter sido aposentado");
    }

    // --- relationships_for / path -------------------------------------------------------------

    /// `upsert_person_to_person`/`relationships_for` resolvem `from`/`to` via
    /// `person_graph::get_person` (nome -> id) — sem um registro upsertado antes, "joao"/"maria"/
    /// "pedro" nunca resolvem a um nó e todo teste abaixo veria `None`/vazio em vez do grafo
    /// semeado. Mesma pegadinha que os testes de `path` mais abaixo (Ada/Bea) já contornavam.
    fn seed_family(mem: &MemoryStore) {
        for name in ["Joao", "Maria", "Pedro"] {
            person_graph::upsert_person(mem, person_graph::UpsertPersonInput { name: name.into(), ..Default::default() }, 1).unwrap();
        }
        // "Meu irmão João trabalha com a Maria na padaria. A Maria é casada com o Pedro, que é o
        // meu chefe." — o exemplo da tarefa, como arestas.
        add_relationship(mem, "joao", "irmao", None, 1).unwrap(); // user -> joao
        add_relationship(mem, "pedro", "chefe", None, 1).unwrap(); // user -> pedro
        upsert_person_to_person(mem, "joao", "maria", "trabalha com", None, 1).unwrap();
        upsert_person_to_person(mem, "maria", "pedro", "casada com", None, 1).unwrap();
    }

    /// Item 4 do pacote "people catalog + relationship graph": uma linha por relação por pessoa —
    /// não uma por PONTA gravada (ver o cabeçalho de [`relationships_for`]: a espelhada existe pro
    /// BFS, não para dobrar a contagem de quem participa da mesma relação duas vezes).
    #[test]
    fn relationships_for_returns_one_row_per_relationship_this_node_participates_in() {
        let mem = MemoryStore::open_in_memory().unwrap();
        seed_family(&mem);

        let maria = relationships_for(&mem, "maria").unwrap();
        assert_eq!(maria.len(), 2, "maria->pedro (direta) + a espelhada de joao->maria, nunca as 2 pontas de cada uma");
        assert!(maria.iter().any(|e| e.to_person_id.as_deref() == Some("pedro")));
        assert!(maria.iter().any(|e| e.to_person_id.as_deref() == Some("joao")), "a espelhada de joao->maria também conta, do lado da maria");
    }

    #[test]
    fn relationships_for_an_unknown_identifier_is_empty_not_an_error() {
        let mem = MemoryStore::open_in_memory().unwrap();
        assert!(relationships_for(&mem, "fantasma").unwrap().is_empty());
    }

    /// "como o João conhece a Maria?" — 1 salto direto (trabalham juntos).
    #[test]
    fn path_finds_a_direct_one_hop_connection() {
        let mem = MemoryStore::open_in_memory().unwrap();
        seed_family(&mem);

        let hops = path(&mem, "joao", "maria", 3).unwrap().expect("caminho direto existe");
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].label, "trabalha com");
    }

    /// "quem é o Pedro [pra mim]?" via `path(user, pedro)` — 1 salto (chefe), preferido ao caminho
    /// mais longo via joao/maria.
    #[test]
    fn path_from_the_user_finds_the_shortest_hop() {
        let mem = MemoryStore::open_in_memory().unwrap();
        seed_family(&mem);

        let hops = path(&mem, "user", "pedro", 3).unwrap().expect("usuário conhece pedro direto");
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].label, "chefe");
    }

    /// Multi-hop: joao -[trabalha com]-> maria -[casada com]-> pedro, 2 saltos, dentro do teto de 3.
    #[test]
    fn path_finds_a_multi_hop_connection_within_the_depth_cap() {
        let mem = MemoryStore::open_in_memory().unwrap();
        for name in ["Joao", "Maria", "Pedro"] {
            person_graph::upsert_person(&mem, person_graph::UpsertPersonInput { name: name.into(), ..Default::default() }, 1).unwrap();
        }
        add_relationship(&mem, "joao", "irmao", None, 1).unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "trabalha com", None, 1).unwrap();
        upsert_person_to_person(&mem, "maria", "pedro", "casada com", None, 1).unwrap();

        // sem aresta direta joao<->pedro: só alcançável em 2 saltos via maria.
        let hops = path(&mem, "joao", "pedro", 3).unwrap().expect("caminho de 2 saltos existe");
        assert_eq!(hops.len(), 2);
        assert_eq!(hops.last().unwrap().to, "pedro");
    }

    #[test]
    fn path_of_the_same_node_is_an_empty_hop_list_not_none() {
        let mem = MemoryStore::open_in_memory().unwrap();
        person_graph::upsert_person(&mem, person_graph::UpsertPersonInput { name: "Ada".into(), ..Default::default() }, 1).unwrap();
        assert_eq!(path(&mem, "Ada", "Ada", 3).unwrap(), Some(vec![]));
    }

    #[test]
    fn path_with_no_connection_within_the_cap_is_none() {
        let mem = MemoryStore::open_in_memory().unwrap();
        person_graph::upsert_person(&mem, person_graph::UpsertPersonInput { name: "Ada".into(), ..Default::default() }, 1).unwrap();
        person_graph::upsert_person(&mem, person_graph::UpsertPersonInput { name: "Bea".into(), ..Default::default() }, 1).unwrap();
        assert_eq!(path(&mem, "Ada", "Bea", 3).unwrap(), None);
    }

    #[test]
    fn path_of_an_unresolvable_identifier_is_none() {
        let mem = MemoryStore::open_in_memory().unwrap();
        assert_eq!(path(&mem, "fantasma-1", "fantasma-2", 3).unwrap(), None);
    }

    #[test]
    fn path_clamps_an_out_of_range_max_depth_to_three() {
        let mem = MemoryStore::open_in_memory().unwrap();
        for name in ["Joao", "Maria", "Pedro", "Ana"] {
            person_graph::upsert_person(&mem, person_graph::UpsertPersonInput { name: name.into(), ..Default::default() }, 1).unwrap();
        }
        add_relationship(&mem, "joao", "irmao", None, 1).unwrap();
        upsert_person_to_person(&mem, "joao", "maria", "trabalha com", None, 1).unwrap();
        upsert_person_to_person(&mem, "maria", "pedro", "casada com", None, 1).unwrap();
        upsert_person_to_person(&mem, "pedro", "ana", "pai de", None, 1).unwrap();

        // 3 saltos (joao->maria->pedro->ana) cabe no teto; pedir 99 não estoura além de 3.
        assert!(path(&mem, "joao", "ana", 99).unwrap().is_some());
    }
}
