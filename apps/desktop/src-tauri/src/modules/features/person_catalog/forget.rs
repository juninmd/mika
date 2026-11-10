//! "Apagar uma pessoa" de verdade: além da ficha do catálogo, o grafo social e a memória episódica
//! que a mencionam. Apagar só a ficha deixava a pessoa lembrada nas conversas guardadas — a UI
//! (`person_catalog_delete`, [`super::rpc`]) e a tool do LLM ([`super::tool`]) passam pelas MESMAS
//! pernas de memória, em [`forget_in_memory`].

use serde_json::{json, Value};

use crate::modules::memory::episodic::EpisodicMemoryService;
use crate::modules::memory::social::SocialMemoryService;
use crate::modules::memory::MemoryStore;

use super::store::CatalogStore;

/// O que as duas pernas de memória fizeram.
pub(super) struct MemoryLegs {
    pub(super) social_deleted: bool,
    pub(super) episodic_removed: usize,
    pub(super) episodic_error: Option<String>,
}

/// A UNIÃO de `terms` (nome e aliases da ficha) com o nome e aliases do perfil social de
/// `identifier`, sem repetidos nem vazios — os termos que a varredura episódica procura.
fn sweep_terms(social: &SocialMemoryService, identifier: &str, mut terms: Vec<String>) -> Vec<String> {
    if let Ok(Some(person)) = social.get_person(identifier) {
        terms.push(person.primary_name.clone());
        terms.extend(person.aliases.clone());
    }
    let mut seen = std::collections::HashSet::new();
    terms.into_iter().filter(|t| !t.is_empty() && seen.insert(t.clone())).collect()
}

/// Esquece `identifier` no grafo social e varre a memória episódica pelos termos de [`sweep_terms`].
/// Os termos são colhidos ANTES de apagar o perfil social, senão o nome some junto com o que o ligava
/// às menções.
pub(super) fn forget_in_memory(mem: &MemoryStore, identifier: &str, terms: Vec<String>) -> MemoryLegs {
    let social = SocialMemoryService::new(mem);
    let unique = sweep_terms(&social, identifier, terms);
    let social_deleted = social.forget_person(identifier).unwrap_or(false);

    let (episodic_removed, episodic_error) = if unique.is_empty() {
        (0, None)
    } else {
        match EpisodicMemoryService::new(mem).forget_mentions(&unique) {
            Ok(removed) => (removed, None),
            Err(error) => (0, Some(error)),
        }
    };
    MemoryLegs { social_deleted, episodic_removed, episodic_error }
}

/// "Esquecer" vindo do painel de memória, que só conhece o NOME que o grafo social mostra: acha a
/// ficha do catálogo (pelo nome ou apelido) e apaga tudo (ficha + grafo social + episódios); sem
/// ficha, ainda varre o grafo social e a memória episódica. `Ok(false)` quando não havia nada.
pub(crate) fn forget_by_name(catalog: Option<&CatalogStore>, mem: &MemoryStore, name: &str) -> Result<bool, String> {
    if let Some(record) = catalog.and_then(|c| c.get(name).or_else(|| c.find_by_alias(name))) {
        delete_everywhere(catalog.expect("ficha achada implica catálogo"), Some(mem), &record.id)?;
        return Ok(true);
    }
    let legs = forget_in_memory(mem, name, vec![name.to_string()]);
    if let Some(error) = legs.episodic_error {
        return Err(error);
    }
    Ok(legs.social_deleted || legs.episodic_removed > 0)
}

/// Prévia de [`delete_everywhere`] para a confirmação da UI: quantas memórias episódicas seriam
/// apagadas, sem apagar NADA (nem a ficha). Apagar é irreversível e o casamento é por nome — o usuário
/// vê o tamanho do estrago antes de confirmar.
pub(super) fn preview_delete(store: &CatalogStore, mem: Option<&MemoryStore>, id: &str) -> Result<Value, String> {
    let record = store.get(id).ok_or_else(|| format!("Person not found: {id}"))?;
    let Some(mem) = mem else {
        return Ok(json!({ "success": true, "data": { "dryRun": true, "episodicRemoved": 0, "warning": "Memory is unavailable right now; only the catalog entry would be erased." } }));
    };
    let mut terms = vec![record.primary_name.clone()];
    terms.extend(record.aliases.clone());
    let terms = sweep_terms(&SocialMemoryService::new(mem), id, terms);
    let episodic = EpisodicMemoryService::new(mem).count_mentions(&terms)?;
    Ok(json!({ "success": true, "data": { "dryRun": true, "episodicRemoved": episodic } }))
}

/// `person_catalog_delete` (a UI): ficha + memória social + episódica. Sem memória aberta
/// (`mem: None`) só a ficha cai, e a resposta diz isso em `warning` em vez de fingir o resto.
pub(super) fn delete_everywhere(store: &CatalogStore, mem: Option<&MemoryStore>, id: &str) -> Result<Value, String> {
    let record = store.get(id);
    store.delete(id)?;
    super::events::publish_deleted(id);

    let mut terms: Vec<String> = Vec::new();
    if let Some(person) = &record {
        terms.push(person.primary_name.clone());
        terms.extend(person.aliases.clone());
    }
    let Some(mem) = mem else {
        return Ok(json!({ "success": true, "data": { "warning": "Memory is unavailable right now; only the catalog entry was erased." } }));
    };
    let legs = forget_in_memory(mem, id, terms);
    let mut data = json!({ "socialDeleted": legs.social_deleted, "episodicRemoved": legs.episodic_removed });
    if let Some(error) = legs.episodic_error {
        data["warning"] = json!(format!("The person was erased, but episodic memory cleanup failed: {error}"));
    }
    Ok(json!({ "success": true, "data": data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::person_catalog::types::UpsertInput;

    fn open_store(tag: &str) -> (CatalogStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mika-person-forget-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        (CatalogStore::open(&dir.join("catalog.json")).unwrap(), dir)
    }

    /// Antes: a UI apagava a ficha e a Mika continuava lembrando "Ana" nas conversas guardadas.
    #[test]
    fn deleting_a_person_also_forgets_her_in_social_and_episodic_memory() {
        let (store, dir) = open_store("everywhere");
        let mem = MemoryStore::open_in_memory().unwrap();
        let ana = store.upsert(UpsertInput { name: "Ana".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        EpisodicMemoryService::new(&mem).store(true, "Ana trabalha com o Pedro", None, None).unwrap();
        EpisodicMemoryService::new(&mem).store(true, "gosta de rust", None, None).unwrap();

        let result = delete_everywhere(&store, Some(&mem), &ana.id).unwrap();

        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["episodicRemoved"], json!(1), "{result}");
        assert!(store.get(&ana.id).is_none());
        let left: Vec<String> = EpisodicMemoryService::new(&mem).get_all().unwrap().into_iter().map(|e| e.content).collect();
        assert_eq!(left, ["gosta de rust"], "só a lembrança que cita Ana sai");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_a_memory_store_the_catalog_entry_still_goes_and_the_response_says_so() {
        let (store, dir) = open_store("nomem");
        let ana = store.upsert(UpsertInput { name: "Ana".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        let result = delete_everywhere(&store, None, &ana.id).unwrap();
        assert_eq!(result["success"], json!(true));
        assert!(result["data"]["warning"].as_str().unwrap().contains("only the catalog entry"));
        assert!(store.get(&ana.id).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_an_unknown_person_is_an_error_and_touches_no_memory() {
        let (store, dir) = open_store("unknown");
        let mem = MemoryStore::open_in_memory().unwrap();
        EpisodicMemoryService::new(&mem).store(true, "Ana mora longe", None, None).unwrap();
        assert!(delete_everywhere(&store, Some(&mem), "pc_inexistente").is_err());
        assert_eq!(EpisodicMemoryService::new(&mem).get_all().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Nome que também é palavra comum: "Link" só leva as memórias que citam o personagem, não as que
    /// dizem "link" no sentido de URL. (Antes casava as duas e a perda era irreversível.)
    #[test]
    fn deleting_a_person_named_like_a_common_word_spares_the_lowercase_uses() {
        let (store, dir) = open_store("common-word");
        let mem = MemoryStore::open_in_memory().unwrap();
        let link = store.upsert(UpsertInput { name: "Link".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        EpisodicMemoryService::new(&mem).store(true, "Link salvou Hyrule", None, None).unwrap();
        EpisodicMemoryService::new(&mem).store(true, "mande o link do vídeo", None, None).unwrap();

        let result = delete_everywhere(&store, Some(&mem), &link.id).unwrap();

        assert_eq!(result["data"]["episodicRemoved"], json!(1), "{result}");
        let left: Vec<String> = EpisodicMemoryService::new(&mem).get_all().unwrap().into_iter().map(|e| e.content).collect();
        assert_eq!(left, ["mande o link do vídeo"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A UI mostra "isto apagará N memórias" ANTES de confirmar: a prévia conta o mesmo que o apagar
    /// removeria e não toca em nada (nem na ficha).
    #[test]
    fn the_preview_reports_the_episodic_count_without_deleting_anything() {
        let (store, dir) = open_store("preview");
        let mem = MemoryStore::open_in_memory().unwrap();
        let ana = store.upsert(UpsertInput { name: "Marina".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        EpisodicMemoryService::new(&mem).store(true, "Marina trabalha com o Pedro", None, None).unwrap();
        EpisodicMemoryService::new(&mem).store(true, "Marina gosta de rock", None, None).unwrap();
        EpisodicMemoryService::new(&mem).store(true, "gosta de rust", None, None).unwrap();

        let result = preview_delete(&store, Some(&mem), &ana.id).unwrap();

        assert_eq!(result["data"]["dryRun"], json!(true));
        assert_eq!(result["data"]["episodicRemoved"], json!(2), "{result}");
        assert!(store.get(&ana.id).is_some(), "prévia não apaga a ficha");
        assert_eq!(EpisodicMemoryService::new(&mem).get_all().unwrap().len(), 3, "prévia não apaga memória");
        let _ = std::fs::remove_dir_all(dir);
    }
}
