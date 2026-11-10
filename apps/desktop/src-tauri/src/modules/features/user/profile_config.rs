//! Efeitos colaterais de `action=forget` no `ConfigStore` — porte de
//! `apps/desktop/src/features/user/handlers/profile-config.ts` +
//! `handlers/learned-entries.ts::clearLearned`/`clearLearnedSelective`. Sem isto, o PII "esquecido"
//! no grafo social ressurgia no próximo `action=get` pela cópia espelhada em `user.*`
//! (`ConfigStore`) — mesma nota do comentário original em `profile-config.ts`.

use serde_json::json;

use crate::config::store::ConfigStore;

use super::USER_PROFILE_KEYS;

/// = `clearProfileConfigField`: volta ao DEFAULT do schema — MESMO mecanismo que
/// `features::privacy::clear::clear_user_profile` já usa (`ConfigStore::set` recusa `null`/`""`
/// que não valide contra o schema; o default É o "vazio" representável de cada campo, inclusive
/// `age`, que não tem string vazia válida — ver o cabeçalho de `mod.rs`).
fn clear_profile_config_field(store: &ConfigStore, key: &str) -> bool {
    let defaults = store.schema().defaults();
    let user_defaults = defaults.get("user").cloned().unwrap_or_else(|| serde_json::Value::Object(Default::default()));
    let default = user_defaults.get(key).cloned().unwrap_or(serde_json::Value::Null);
    store.set(&format!("user.{key}"), default).is_ok()
}

/// = `clearProfileConfigFields`. `keys = None` limpa TODOS os `USER_PROFILE_KEYS` (default do TS
/// quando chamado sem segundo argumento).
pub fn clear_profile_config_fields(store: &ConfigStore, keys: Option<&[&str]>) -> bool {
    let keys = keys.unwrap_or(USER_PROFILE_KEYS);
    keys.iter().fold(true, |ok, key| clear_profile_config_field(store, key) && ok)
}

/// = `FORGET_FIELD_TO_CONFIG_KEYS` + `configKeysForForgetFields`: mapeia `forget(fields)` para as
/// chaves `user.*` equivalentes — `pronouns` não tem cópia no `ConfigStore`, mesma lacuna do TS.
pub fn config_keys_for_forget_fields(fields: &[String]) -> Vec<&'static str> {
    let mut keys = Vec::new();
    for field in fields {
        let mapped: &[&str] = match field.as_str() {
            "preferredName" => &["name"],
            "timezone" => &["timezone"],
            "personality" => &["personalityTraits", "communicationStyle", "communicationNotes"],
            _ => &[],
        };
        for key in mapped {
            if !keys.contains(key) {
                keys.push(*key);
            }
        }
    }
    keys
}

/// = `clearLearned`.
pub fn clear_learned(store: &ConfigStore) -> bool {
    store.set("user.learnedByLLM", json!("")).is_ok()
}

/// = `FORGET_FIELD_TO_LEARNED_CATEGORIES`: mapeia `forget(fields)` para as tags de
/// `learn(category=...)` cujas linhas de `user.learnedByLLM` também precisam sumir.
fn learned_categories_for(fields: &[String]) -> Vec<&'static str> {
    let mut categories = Vec::new();
    for field in fields {
        let mapped: &[&str] = match field.as_str() {
            "preferredName" => &["name"],
            "personality" => &["personality"],
            _ => &[],
        };
        for category in mapped {
            if !categories.contains(category) {
                categories.push(*category);
            }
        }
    }
    categories
}

/// = `LEARNED_BULLET_RE` (`/^- \[.*?\]\s*\((\w+)\):\s*(.*)$/`): recorta a linha que `run_learn`
/// grava (`- [data] (categoria): fato`). `None` para qualquer linha fora desse formato — TRATADA
/// como "não é bullet, preserva" pelo chamador, nunca como erro.
fn parse_learned_bullet(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("- [")?;
    let (_, rest) = rest.split_once(']')?;
    let rest = rest.trim_start().strip_prefix('(')?;
    let (category, rest) = rest.split_once(')')?;
    let fact = rest.strip_prefix(':')?.trim_start();
    Some((category, fact))
}

/// = `filterLearnedEntries`: best-effort por palavra-chave, não NLP — uma linha sobrevive a menos
/// que sua categoria ou o texto do fato citem o campo/fato esquecido, MESMA heurística do TS.
fn filter_learned_entries(text: &str, categories: &[&str], fact_terms: &[String]) -> String {
    text.lines()
        .filter(|line| {
            let Some((category, fact)) = parse_learned_bullet(line) else { return true };
            if categories.contains(&category) {
                return false;
            }
            let fact_lower = fact.to_lowercase();
            !fact_terms.iter().any(|term| fact_lower.contains(term.as_str()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// = `clearLearnedSelective`.
pub fn clear_learned_selective(store: &ConfigStore, fields: &[String], facts: &[String]) -> bool {
    let current = store.get("user.learnedByLLM").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    if current.is_empty() {
        return false;
    }
    let categories = learned_categories_for(fields);
    let fact_terms: Vec<String> = facts.iter().map(|f| f.trim().to_lowercase()).filter(|f| !f.is_empty()).collect();
    if categories.is_empty() && fact_terms.is_empty() {
        return false;
    }
    let filtered = filter_learned_entries(&current, &categories, &fact_terms);
    if filtered == current {
        return false;
    }
    store.set("user.learnedByLLM", json!(filtered)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{int, text};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-profile-config-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "user",
            fields: vec![text("name", ""), text("timezone", ""), text("learnedByLLM", ""), int("age", 1)],
        }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    #[test]
    fn clear_profile_config_fields_resets_every_key_to_its_schema_default() {
        let (store, _dir) = store();
        store.set("user.name", json!("Ana")).unwrap();
        store.set("user.age", json!(30)).unwrap();

        assert!(clear_profile_config_fields(&store, Some(&["name", "age"])));
        assert_eq!(store.get("user.name"), Some(json!("")));
        assert_eq!(store.get("user.age"), Some(json!(1)), "age não tem vazio representável, volta ao mínimo do schema");
    }

    #[test]
    fn config_keys_for_forget_fields_maps_known_fields_and_drops_pronouns() {
        let keys = config_keys_for_forget_fields(&["preferredName".into(), "pronouns".into(), "timezone".into()]);
        assert!(keys.contains(&"name"));
        assert!(keys.contains(&"timezone"));
        assert_eq!(keys.len(), 2, "pronouns não tem chave de config equivalente");
    }

    #[test]
    fn clear_learned_wipes_the_whole_field() {
        let (store, _dir) = store();
        store.set("user.learnedByLLM", json!("- [2024-01-01] (name): Ana")).unwrap();
        assert!(clear_learned(&store));
        assert_eq!(store.get("user.learnedByLLM"), Some(json!("")));
    }

    #[test]
    fn clear_learned_selective_drops_only_bullets_of_the_named_category() {
        let (store, _dir) = store();
        store
            .set(
                "user.learnedByLLM",
                json!("- [2024-01-01] (name): Ana\n- [2024-01-01] (profession): engenheira"),
            )
            .unwrap();

        assert!(clear_learned_selective(&store, &["preferredName".into()], &[]));
        let left = store.get("user.learnedByLLM").unwrap();
        assert!(!left.as_str().unwrap().contains("(name)"), "bullet de name devia sumir: {left}");
        assert!(left.as_str().unwrap().contains("(profession)"), "bullet de profession não devia sumir: {left}");
    }

    /// NEGATIVO — sem categoria nem fato pra remover, não escreve nada (e devolve `false`).
    #[test]
    fn clear_learned_selective_is_a_no_op_without_a_matching_category_or_fact() {
        let (store, _dir) = store();
        store.set("user.learnedByLLM", json!("- [2024-01-01] (hobby): games")).unwrap();

        assert!(!clear_learned_selective(&store, &["pronouns".into()], &[]));
        assert_eq!(store.get("user.learnedByLLM"), Some(json!("- [2024-01-01] (hobby): games")));
    }
}
