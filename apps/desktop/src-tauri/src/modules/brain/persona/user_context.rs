//! === user-profile-context.ts =====================================================================
//!
//! O bloco "USER PROFILE AND HABITS" que `system_prompt::build_volatile_context` anexa —
//! porte 1:1 dos rótulos e da composição de `buildUserProfileContext`/`buildPartnerContext`.
//! As chaves lidas são as MESMAS de `features::user::USER_PROFILE_KEYS`
//! (`modules/features/user/tool.rs`) — reusadas, não reinventadas, para as duas listas nunca
//! divergirem em silêncio.

use crate::config::store::ConfigStore;
use crate::modules::features::user::USER_PROFILE_KEYS;

const PARTNER_KEYS: &[&str] = &["partnerName", "partnerRelationship", "partnerLikes", "partnerNotes"];

/// Espelha `USER_FIELD_LABELS`, na mesma ordem — `USER_PROFILE_KEYS` inclui os 4 `PARTNER_KEYS`
/// (compostos à parte em [`partner_context`]), por isso ficam de fora deste rótulo direto.
fn label_for(key: &str) -> Option<&'static str> {
    Some(match key {
        "name" => "Nome",
        "nickname" => "Apelido",
        "birthDate" => "Data de nascimento",
        "age" => "Idade",
        "gender" => "Genero",
        "country" => "Pais",
        "state" => "Estado",
        "city" => "Cidade",
        "timezone" => "Fuso horario",
        "profession" => "Profissao",
        "company" => "Empresa",
        "role" => "Cargo",
        "skills" => "Habilidades",
        "hobbies" => "Hobbies",
        "favoriteGames" => "Jogos favoritos",
        "favoriteAnime" => "Animes/filmes/series favoritos",
        "favoriteMusic" => "Musica",
        "favoriteFood" => "Comida favorita",
        "personalityTraits" => "Tracos de personalidade",
        "communicationStyle" => "Estilo de comunicacao",
        "communicationNotes" => "Notas de comunicacao",
        "bio" => "Bio",
        "notes" => "Notas livres",
        "learnedByLLM" => "Aprendido pela IA",
        _ => return None,
    })
}

/// `age` não tem vazio representável no schema Rust: o default `1` é o sentinela do `undefined` do
/// TS (ver o comentário do campo em `config/schemas/user.rs`). Sem este corte, todo perfil novo
/// diria ao modelo "Idade: 1" — dado inventado, não do usuário.
const AGE_UNSET_SENTINEL: u64 = 1;

fn field_text(store: &ConfigStore, key: &str) -> String {
    match store.get(&format!("user.{key}")) {
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Number(n)) if key == "age" && n.as_u64() == Some(AGE_UNSET_SENTINEL) => String::new(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Espelha `buildPartnerContext`. Mantém o mapeamento `"marida" -> "marido"` do TS tal qual —
/// porte fiel, não correção; mudar essa regra é decisão de produto, fora do escopo deste port.
fn partner_context(store: &ConfigStore) -> String {
    let name = field_text(store, "partnerName");
    if name.is_empty() {
        return String::new();
    }
    let relationship_raw = field_text(store, "partnerRelationship");
    let relationship = if relationship_raw.is_empty() { "parceira".to_string() } else if relationship_raw == "marida" { "marido".to_string() } else { relationship_raw };
    let likes = field_text(store, "partnerLikes");
    let notes = field_text(store, "partnerNotes");

    let mut parts = vec![format!("{relationship}: {name}")];
    if !likes.is_empty() {
        parts.push(format!("gosta de {likes}"));
    }
    if !notes.is_empty() {
        parts.push(notes);
    }
    parts.join("; ")
}

/// Espelha `buildUserProfileContext` — uma linha `- Rótulo: valor` por campo preenchido, na ordem
/// de `USER_PROFILE_KEYS`, mais a linha "Pessoa parceira" quando houver `partnerName`.
pub fn build_user_profile_context(store: &ConfigStore) -> String {
    let mut lines = Vec::new();

    for key in USER_PROFILE_KEYS {
        if PARTNER_KEYS.contains(key) {
            continue;
        }
        let Some(label) = label_for(key) else { continue };
        let text = field_text(store, key);
        if !text.is_empty() {
            lines.push(format!("- {label}: {text}"));
        }
    }

    let partner = partner_context(store);
    if !partner.is_empty() {
        lines.push(format!("- Pessoa parceira: {partner}"));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct ConfigDir(PathBuf);
    impl ConfigDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-persona-user-context-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            ConfigDir(path)
        }
    }
    impl Drop for ConfigDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, ConfigDir) {
        let dir = ConfigDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (store, dir)
    }

    #[test]
    fn build_user_profile_context_is_empty_on_a_fresh_store() {
        let (store, _dir) = store();
        assert_eq!(build_user_profile_context(&store), "");
    }

    #[test]
    fn build_user_profile_context_reports_a_real_age_but_never_the_unset_sentinel() {
        let (store, _dir) = store();
        assert!(!build_user_profile_context(&store).contains("Idade"), "idade não informada não pode virar 'Idade: 1'");
        store.set("user.age", serde_json::json!(30)).unwrap();
        assert!(build_user_profile_context(&store).contains("- Idade: 30"));
    }

    #[test]
    fn build_user_profile_context_includes_only_filled_fields_with_the_right_label() {
        let (store, _dir) = store();
        store.set("user.name", serde_json::json!("Ana")).unwrap();
        store.set("user.favoriteGames", serde_json::json!("Hollow Knight")).unwrap();

        let ctx = build_user_profile_context(&store);
        assert!(ctx.contains("- Nome: Ana"));
        assert!(ctx.contains("- Jogos favoritos: Hollow Knight"));
        assert!(!ctx.contains("Apelido"));
    }

    #[test]
    fn build_user_profile_context_composes_the_partner_line_and_applies_the_legacy_gender_swap() {
        let (store, _dir) = store();
        store.set("user.partnerName", serde_json::json!("Joao")).unwrap();
        store.set("user.partnerRelationship", serde_json::json!("marida")).unwrap();
        store.set("user.partnerLikes", serde_json::json!("cafe")).unwrap();

        let ctx = build_user_profile_context(&store);
        assert!(ctx.contains("- Pessoa parceira: marido: Joao; gosta de cafe"), "ctx: {ctx}");
    }

    #[test]
    fn build_user_profile_context_omits_the_partner_line_without_a_partner_name() {
        let (store, _dir) = store();
        store.set("user.partnerLikes", serde_json::json!("cafe")).unwrap();
        assert!(!build_user_profile_context(&store).contains("Pessoa parceira"));
    }
}
