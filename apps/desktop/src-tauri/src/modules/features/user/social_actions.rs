//! Ações de escrita social da tool `user_profile` — porte de `handlers/user-actions.ts`
//! (`runSet`/`runForget`/`runSetPersonality`) + o efeito colateral de `runLearn`
//! (`fillPreferredNameIfEmpty`). `run_get`/`run_learn` (só `ConfigStore`) ficam em `tool.rs`; aqui
//! só o que precisa de [`SocialMemoryService`].
//!
//! `fillPreferredNameIfEmpty` no TS é fire-and-forget (`void fillPreferredNameIfEmpty(fact)`,
//! não esperado pelo chamador) e carrega um contador `profileGeneration` só para invalidar uma
//! chamada em voo se um `forget` correr no meio do caminho. Aqui a escrita roda SÍNCRONA, antes da
//! tool devolver a resposta de `learn` — sem `await` no meio, não existe janela pra uma corrida, e
//! o contador de geração vira desnecessário. Divergência de TIMING (a resposta espera pela escrita
//! auxiliar, µs de diferença), nunca de resultado observável.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::memory::social::types::UserProfile;
use crate::modules::memory::social::{ForgetOutcome, ForgetUserProfileInput, PersonalityInput, SetUserProfileInput, SocialMemoryService};

use super::profile_config::{clear_learned, clear_learned_selective, clear_profile_config_fields, config_keys_for_forget_fields};

const MEMORY_DISABLED_MESSAGE: &str = "Memory recording is disabled (memory.enabled=false); nothing was saved.";

/// = `MEMORY_DISABLED` (`handlers/user-result.ts`).
fn memory_disabled() -> Value {
    json!({ "success": false, "error": MEMORY_DISABLED_MESSAGE, "errorCode": "denied" })
}

fn social_error(error: String) -> Value {
    json!({ "success": false, "error": error, "errorCode": "unavailable" })
}

fn profile_to_json(profile: &UserProfile) -> Value {
    serde_json::to_value(profile).unwrap_or_else(|_| json!({}))
}

pub(super) fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(super) fn str_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key).and_then(Value::as_array).map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// = `runSet`.
pub fn handle_set(social: &SocialMemoryService<'_>, recording_allowed: bool, args: &Value, now_ms: i64) -> Value {
    let input = SetUserProfileInput {
        preferred_name: opt_str(args, "preferredName"),
        pronouns: opt_str(args, "pronouns"),
        timezone: opt_str(args, "timezone"),
        languages: str_array(args, "languages"),
        facts: str_array(args, "facts"),
        personality: None,
    };
    match social.set_user_profile(recording_allowed, input, now_ms) {
        Ok(Some(profile)) => json!({ "success": true, "data": { "profile": profile_to_json(&profile) } }),
        Ok(None) => memory_disabled(),
        Err(error) => social_error(error),
    }
}

/// = `runForget`. Também aciona os efeitos colaterais no `ConfigStore`
/// (`clearProfileConfigFields`/`clearLearned*`) — sem isso o PII "esquecido" no grafo social
/// ressurgia no próximo turno pela cópia espelhada em `user.*` (mesma nota do TS em
/// `profile-config.ts`). Roda mesmo quando não havia perfil social pra esquecer
/// (`ForgetOutcome::NothingToForget`) — MESMA ordem do TS, que só pula os efeitos colaterais
/// quando `forgetUserProfile` LANÇA (`MemoryRecordingDisabledError`), nunca quando devolve `null`.
pub fn handle_forget(store: &ConfigStore, social: &SocialMemoryService<'_>, recording_allowed: bool, args: &Value, now_ms: i64) -> Value {
    let fields = str_array(args, "fields");
    let facts = str_array(args, "facts");
    let cleared_all = args.get("all").and_then(Value::as_bool).unwrap_or(false);
    let input = ForgetUserProfileInput { fields: fields.clone(), facts: facts.clone(), languages: str_array(args, "languages"), all: cleared_all };

    let outcome = match social.forget_user_profile(recording_allowed, input, now_ms) {
        Ok(ForgetOutcome::Blocked) => return memory_disabled(),
        Ok(outcome) => outcome,
        Err(error) => return social_error(error),
    };
    let profile = match &outcome {
        ForgetOutcome::Forgotten(p) => profile_to_json(p),
        ForgetOutcome::NothingToForget => Value::Null,
        ForgetOutcome::Blocked => unreachable!("tratado acima"),
    };

    let target_config_keys = if cleared_all { Vec::new() } else { config_keys_for_forget_fields(&fields) };
    let learned_cleared = if cleared_all { clear_learned(store) } else { clear_learned_selective(store, &fields, &facts) };
    let profile_config_cleared = if cleared_all {
        clear_profile_config_fields(store, None)
    } else {
        !target_config_keys.is_empty() && clear_profile_config_fields(store, Some(&target_config_keys))
    };

    json!({
        "success": true,
        "data": { "profile": profile, "learnedCleared": learned_cleared, "profileConfigCleared": profile_config_cleared },
    })
}

/// = `runSetPersonality`.
pub fn handle_set_personality(social: &SocialMemoryService<'_>, recording_allowed: bool, args: &Value, now_ms: i64) -> Value {
    let input = PersonalityInput {
        communication_style: opt_str(args, "communicationStyle"),
        humor_tolerance: opt_str(args, "humorTolerance"),
        energy_level: opt_str(args, "energyLevel"),
        top_interests: str_array(args, "topInterests"),
    };
    match social.set_personality_profile(recording_allowed, input, now_ms) {
        Ok(Some(profile)) => json!({ "success": true, "data": { "profile": profile_to_json(&profile) } }),
        Ok(None) => memory_disabled(),
        Err(error) => social_error(error),
    }
}

/// = `fillPreferredNameIfEmpty`: best-effort — falha (bloqueado por `memory.enabled=false`, ou erro
/// de I/O) é descartada, igual ao `catch {}` do TS. A escrita que IMPORTA (`user.name` no
/// `ConfigStore`) já aconteceu antes desta função ser chamada; isto só preenche o perfil social se
/// ele ainda não tiver um nome preferido.
pub fn fill_preferred_name_if_empty(social: &SocialMemoryService<'_>, recording_allowed: bool, name: &str, now_ms: i64) {
    let has_preferred_name =
        social.get_user_profile().ok().flatten().and_then(|p| p.preferred_name).filter(|n| !n.is_empty()).is_some();
    if has_preferred_name {
        return;
    }
    let _ = social.set_user_profile(recording_allowed, SetUserProfileInput { preferred_name: Some(name.to_string()), ..Default::default() }, now_ms);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::text;
    use crate::modules::memory::MemoryStore;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-social-actions-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `clear_profile_config_fields(store, None)` (o caminho de `forget(all: true)`) percorre TODA
    /// `USER_PROFILE_KEYS`, não só os campos que o teste usa — em produção `config/schemas/user.rs`
    /// já declara as 27 chaves. Um schema de teste que declarasse só 3 faria o `fold` AND falhar
    /// nas chaves ausentes (`caminho não declarado`) e `profileConfigCleared` sair `false` por um
    /// buraco do FIXTURE, não do produto — foi exatamente isso que a primeira versão deste teste
    /// escondia.
    fn config_store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let fields = super::super::tool::USER_PROFILE_KEYS.iter().map(|key| text(*key, "")).collect();
        let schema = SchemaRegistry::new(vec![Namespace { name: "user", fields }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    fn mem() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn handle_set_dispatches_for_real_and_returns_the_written_profile() {
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        let result = handle_set(&social, true, &json!({ "preferredName": "Ada", "languages": ["pt"] }), 1);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["profile"]["preferredName"], json!("Ada"));
        assert_eq!(social.get_user_profile().unwrap().unwrap().preferred_name.as_deref(), Some("Ada"));
    }

    /// NEGATIVO — `memory.enabled=false` recusa nomeado, nada é gravado.
    #[test]
    fn handle_set_blocked_by_memory_enabled_false_persists_nothing_and_names_the_cause() {
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        let result = handle_set(&social, false, &json!({ "preferredName": "Ada" }), 1);
        assert_eq!(result["errorCode"], json!("denied"));
        assert!(social.get_user_profile().unwrap().is_none());
    }

    #[test]
    fn handle_forget_all_wipes_the_social_profile_and_the_config_mirror_together() {
        let (config, _dir) = config_store();
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        handle_set(&social, true, &json!({ "preferredName": "Ada" }), 1);
        config.set("user.name", json!("Ada")).unwrap();

        let result = handle_forget(&config, &social, true, &json!({ "all": true }), 2);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["profile"]["preferredName"], Value::Null);
        // `clear_learned` (= TS `clearLearned`, `learned-entries.ts:21-28`) é INCONDICIONAL — sempre
        // grava "" e devolve `true` no sucesso, ao contrário de `clear_learned_selective` que checa
        // "já vazio" antes. `all: true` chama `clear_learned`, não a versão seletiva.
        assert_eq!(result["data"]["learnedCleared"], json!(true), "clear_learned é incondicional, sempre true no sucesso");
        assert_eq!(result["data"]["profileConfigCleared"], json!(true));
        assert_eq!(config.get("user.name"), Some(json!("")), "cópia no ConfigStore tinha de ser limpa junto");
        assert!(social.get_user_profile().unwrap().unwrap().preferred_name.is_none());
    }

    /// Sem perfil social pra esquecer, `forget` ainda é `success: true` (NO-OP honesto, não erro) —
    /// e ainda roda os efeitos colaterais de config (mesma ordem do TS).
    #[test]
    fn handle_forget_without_a_social_profile_still_succeeds_and_runs_config_side_effects() {
        let (config, _dir) = config_store();
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        config.set("user.name", json!("Ada")).unwrap();

        let result = handle_forget(&config, &social, true, &json!({ "fields": ["preferredName"] }), 1);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["profile"], Value::Null);
        assert_eq!(config.get("user.name"), Some(json!("")), "efeito colateral de config roda mesmo sem perfil social");
    }

    /// NEGATIVO — perfil social existente + `memory.enabled=false`: recusa nomeado, e os efeitos
    /// colaterais de config NÃO rodam (mesma ordem do `try/catch` do TS).
    #[test]
    fn handle_forget_of_an_existing_profile_blocked_by_memory_enabled_false_skips_config_side_effects() {
        let (config, _dir) = config_store();
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        handle_set(&social, true, &json!({ "preferredName": "Ada" }), 1);
        config.set("user.name", json!("Ada")).unwrap();

        let result = handle_forget(&config, &social, false, &json!({ "all": true }), 2);
        assert_eq!(result["errorCode"], json!("denied"));
        assert_eq!(config.get("user.name"), Some(json!("Ada")), "bloqueado não pode ter mexido na cópia de config");
    }

    #[test]
    fn handle_set_personality_dispatches_for_real() {
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        let result = handle_set_personality(&social, true, &json!({ "communicationStyle": "playful" }), 1);
        assert_eq!(result["data"]["profile"]["personality"]["communicationStyle"], json!("playful"));
    }

    #[test]
    fn fill_preferred_name_if_empty_fills_a_blank_social_profile() {
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        fill_preferred_name_if_empty(&social, true, "Ada", 1);
        assert_eq!(social.get_user_profile().unwrap().unwrap().preferred_name.as_deref(), Some("Ada"));
    }

    /// NEGATIVO — já existe `preferredName`, não sobrescreve.
    #[test]
    fn fill_preferred_name_if_empty_never_overwrites_an_existing_one() {
        let memory = mem();
        let social = SocialMemoryService::new(&memory);
        handle_set(&social, true, &json!({ "preferredName": "Original" }), 1);
        fill_preferred_name_if_empty(&social, true, "Outro", 2);
        assert_eq!(social.get_user_profile().unwrap().unwrap().preferred_name.as_deref(), Some("Original"));
    }
}
