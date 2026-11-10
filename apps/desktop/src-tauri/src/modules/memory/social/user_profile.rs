//! Escritas do perfil social "estável" — porte de
//! `apps/desktop/src/main/services/memory/social/user-profile-store.ts`. Único ponto do módulo que
//! grava em `SOCIAL_TABLE_USER_PROFILE`; `SocialMemoryService` (`mod.rs`) só delega para cá.
//!
//! ## Semântica do merge, medida linha a linha no TS (não copiada de `add_fact`/`add_relationship`
//! vizinhos — o perfil se comporta diferente deles)
//! - `setUserProfile` (`user-profile-store.ts:14-32`): `preferredName`/`pronouns`/`timezone` são
//!   SUBSTITUÍDOS quando o campo está presente no input (mesmo `""` — o `??` do JS só cai no
//!   fallback com `null`/`undefined`) e PRESERVADOS quando ausente — um `set` parcial nunca zera o
//!   que não menciona. `languages`/`facts` são ACUMULADOS (concat + dedup por
//!   `compactUnique`/`compact_unique`, teto 8/50), nunca substituídos — um `set` com 1 idioma novo
//!   não apaga os já gravados. `facts` passa por `redactSecrets` (segredo fora, PII social
//!   preservada — mesma redação seletiva de `add_fact`) ANTES do dedup.
//! - `forgetUserProfile` (`user-profile-store.ts:34-72`): sem perfil gravado é NO-OP — devolve
//!   `null` sem sequer olhar `memory.enabled` (nada a bloquear, o gate só existe para proteger uma
//!   ESCRITA). `all=true` SOBRESCREVE para `{id, languages: [], facts: [], updatedAt}`: os DADOS
//!   somem, mas o REGISTRO continua existindo (upsert, nunca delete — `get_graph().profile` volta
//!   `Some(perfil vazio)`, não `None`). `fields` apaga campos nomeados um a um; `languages`/`facts`
//!   removem só os valores citados (por `normalizeText`), preservando o resto.
//! - `setPersonalityProfile` (`user-profile-store.ts:74-87`): funde a personalidade (substitui nos
//!   3 campos escalares, acumula `topInterests`) e delega para `setUserProfile` passando SÓ
//!   `personality` — os demais campos do perfil (nome, idiomas, fatos...) ficam intocados, porque
//!   `setUserProfile` os trata como ausentes quando o input não os menciona.

use crate::modules::memory::MemoryStore;

use crate::modules::memory::redact::redact_for_storage;
use super::rules::{compact_unique, normalize_text};
use super::store;
use super::types::{PersonalityProfile, UserProfile, SOCIAL_TABLE_USER_PROFILE};

const USER_ID: &str = "user";
const MAX_LANGUAGES: usize = 8;
const MAX_FACTS: usize = 50;
const MAX_TOP_INTERESTS: usize = 20;

/// = `getUserProfile`.
pub fn get_user_profile(mem: &MemoryStore) -> Result<Option<UserProfile>, String> {
    store::get_by_id(mem, SOCIAL_TABLE_USER_PROFILE, USER_ID)
}

/// Entrada de `setUserProfile` — campo ausente (`None`/`vec![]`) = "não mencionado", igual a
/// `undefined` do lado JS. `#[derive(Default)]` é o que [`set_personality_profile`] usa para
/// mandar só `personality` sem tocar no resto.
#[derive(Default)]
pub struct SetUserProfileInput {
    pub preferred_name: Option<String>,
    pub pronouns: Option<String>,
    pub timezone: Option<String>,
    pub languages: Vec<String>,
    pub facts: Vec<String>,
    pub personality: Option<PersonalityProfile>,
}

/// = `setUserProfile`. `Ok(None)` só quando `memory.enabled=false` bloqueia a escrita — MESMO
/// contrato de [`super::SocialMemoryService::add_fact`] (nunca finge sucesso).
pub fn set_user_profile(
    mem: &MemoryStore,
    recording_allowed: bool,
    input: SetUserProfileInput,
    now_ms: i64,
) -> Result<Option<UserProfile>, String> {
    if !recording_allowed {
        return Ok(None);
    }
    let current = get_user_profile(mem)?;

    let mut languages = current.as_ref().map_or_else(Vec::new, |p| p.languages.clone());
    languages.extend(input.languages);

    let mut facts = current.as_ref().map_or_else(Vec::new, |p| p.facts.clone());
    facts.extend(input.facts.iter().map(|f| redact_for_storage(f)));

    let profile = UserProfile {
        id: USER_ID.to_string(),
        preferred_name: input.preferred_name.or_else(|| current.as_ref().and_then(|p| p.preferred_name.clone())),
        pronouns: input.pronouns.or_else(|| current.as_ref().and_then(|p| p.pronouns.clone())),
        timezone: input.timezone.or_else(|| current.as_ref().and_then(|p| p.timezone.clone())),
        languages: compact_unique(languages, MAX_LANGUAGES),
        facts: compact_unique(facts, MAX_FACTS),
        personality: input.personality.or_else(|| current.and_then(|p| p.personality)),
        updated_at: now_ms,
    };
    store::upsert(mem, SOCIAL_TABLE_USER_PROFILE, USER_ID, now_ms as f64, &profile)?;
    Ok(Some(profile))
}

/// Entrada de `forgetUserProfile`. `fields` guarda os MESMOS literais que o schema da tool aceita
/// (`preferredName`/`pronouns`/`timezone`/`personality`) como string crua — igual ao TS, que só os
/// tipa como union no compilador; em runtime é comparação de string também.
#[derive(Default)]
pub struct ForgetUserProfileInput {
    pub fields: Vec<String>,
    pub facts: Vec<String>,
    pub languages: Vec<String>,
    pub all: bool,
}

/// TS devolve `UserProfile | null`, mas `null` tem DOIS significados que o chamador da tool precisa
/// distinguir: nada pra esquecer (sucesso vazio) vs. bloqueado por `memory.enabled=false`
/// (`errorCode: denied`, mesmo caso de [`MEMORY_DISABLED`] no TS) — juntar os dois sob `None`
/// esconderia qual dos dois aconteceu.
pub enum ForgetOutcome {
    NothingToForget,
    Blocked,
    Forgotten(UserProfile),
}

/// = `forgetUserProfile`. Sem perfil gravado é NO-OP — nem olha `recording_allowed`, MESMA ordem
/// do TS (`if (!current) return null` roda antes do gate de `persist`).
pub fn forget_user_profile(
    mem: &MemoryStore,
    recording_allowed: bool,
    input: ForgetUserProfileInput,
    now_ms: i64,
) -> Result<ForgetOutcome, String> {
    let Some(current) = get_user_profile(mem)? else {
        return Ok(ForgetOutcome::NothingToForget);
    };
    if !recording_allowed {
        return Ok(ForgetOutcome::Blocked);
    }

    let drop = |field: &str| input.fields.iter().any(|f| f == field);
    let profile = if input.all {
        UserProfile { id: USER_ID.to_string(), preferred_name: None, pronouns: None, timezone: None, languages: Vec::new(), facts: Vec::new(), personality: None, updated_at: now_ms }
    } else {
        UserProfile {
            id: USER_ID.to_string(),
            preferred_name: if drop("preferredName") { None } else { current.preferred_name },
            pronouns: if drop("pronouns") { None } else { current.pronouns },
            timezone: if drop("timezone") { None } else { current.timezone },
            languages: remove_matching(current.languages, &input.languages),
            facts: remove_matching(current.facts, &input.facts),
            personality: if drop("personality") { None } else { current.personality },
            updated_at: now_ms,
        }
    };
    store::upsert(mem, SOCIAL_TABLE_USER_PROFILE, USER_ID, now_ms as f64, &profile)?;
    Ok(ForgetOutcome::Forgotten(profile))
}

/// = `dropped` (`user-profile-store.ts:54-58`): remove por `normalizeText`, devolve a lista intacta
/// quando não há nada pra remover — nunca zera `languages`/`facts` por engano num `forget(fields)`
/// que não os cita.
fn remove_matching(values: Vec<String>, remove: &[String]) -> Vec<String> {
    if remove.is_empty() {
        return values;
    }
    let targets: std::collections::HashSet<String> = remove.iter().map(|v| normalize_text(v)).collect();
    values.into_iter().filter(|v| !targets.contains(&normalize_text(v))).collect()
}

/// Entrada de `setPersonalityProfile`.
#[derive(Default)]
pub struct PersonalityInput {
    pub communication_style: Option<String>,
    pub humor_tolerance: Option<String>,
    pub energy_level: Option<String>,
    pub top_interests: Vec<String>,
}

/// = `setPersonalityProfile`: funde a personalidade e delega para [`set_user_profile`] passando só
/// `personality` — os outros campos do perfil ficam intocados (o `SetUserProfileInput::default()`
/// os deixa `None`/vazios, que `set_user_profile` trata como "não mencionado").
pub fn set_personality_profile(
    mem: &MemoryStore,
    recording_allowed: bool,
    input: PersonalityInput,
    now_ms: i64,
) -> Result<Option<UserProfile>, String> {
    let current_personality = get_user_profile(mem)?.and_then(|p| p.personality);

    let mut top_interests = current_personality.as_ref().map_or_else(Vec::new, |p| p.top_interests.clone());
    top_interests.extend(input.top_interests);

    let merged = PersonalityProfile {
        communication_style: input.communication_style.or_else(|| current_personality.as_ref().and_then(|p| p.communication_style.clone())),
        humor_tolerance: input.humor_tolerance.or_else(|| current_personality.as_ref().and_then(|p| p.humor_tolerance.clone())),
        energy_level: input.energy_level.or_else(|| current_personality.as_ref().and_then(|p| p.energy_level.clone())),
        top_interests: compact_unique(top_interests, MAX_TOP_INTERESTS),
    };

    set_user_profile(mem, recording_allowed, SetUserProfileInput { personality: Some(merged), ..Default::default() }, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-user-profile-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn set_user_profile_fills_a_fresh_profile() {
        let mem = store();
        let input = SetUserProfileInput { preferred_name: Some("Ada".into()), languages: vec!["pt".into()], ..Default::default() };
        let profile = set_user_profile(&mem, true, input, 1).unwrap().unwrap();
        assert_eq!(profile.preferred_name.as_deref(), Some("Ada"));
        assert_eq!(profile.languages, vec!["pt".to_string()]);
    }

    /// Prova central da tarefa: um `set` que só menciona `pronouns` NÃO pode apagar `preferredName`
    /// já gravado — a classe de defeito que a degradação honesta de antes evitava por completo.
    #[test]
    fn a_partial_set_never_erases_fields_it_does_not_mention() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();

        let profile = set_user_profile(&mem, true, SetUserProfileInput { pronouns: Some("she/her".into()), ..Default::default() }, 2).unwrap().unwrap();
        assert_eq!(profile.preferred_name.as_deref(), Some("Ada"), "set parcial apagou um campo que nem mencionou");
        assert_eq!(profile.pronouns.as_deref(), Some("she/her"));
    }

    #[test]
    fn languages_and_facts_accumulate_instead_of_being_replaced() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { languages: vec!["pt".into()], ..Default::default() }, 1).unwrap();
        let profile = set_user_profile(&mem, true, SetUserProfileInput { languages: vec!["en".into()], ..Default::default() }, 2).unwrap().unwrap();
        assert_eq!(profile.languages, vec!["pt".to_string(), "en".to_string()], "segundo set apagou o idioma do primeiro");
    }

    #[test]
    fn facts_are_deduped_keeping_the_most_recent_spelling() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { facts: vec!["gosta de rust".into()], ..Default::default() }, 1).unwrap();
        let profile = set_user_profile(&mem, true, SetUserProfileInput { facts: vec!["Gosta de Rust".into()], ..Default::default() }, 2).unwrap().unwrap();
        assert_eq!(profile.facts, vec!["Gosta de Rust".to_string()], "duplicata por normalizeText tinha de deduplicar, mantendo a grafia mais recente");
    }

    /// Segredo é redigido, PII social não — mesma redação seletiva de `add_fact`.
    #[test]
    fn set_user_profile_redacts_secrets_from_facts_but_keeps_social_pii() {
        let mem = store();
        let key = format!("sk-ant-{}", "a".repeat(40));
        let profile = set_user_profile(
            &mem,
            true,
            SetUserProfileInput { facts: vec![format!("chave {key}"), "email ada@example.com".into()], ..Default::default() },
            1,
        )
        .unwrap()
        .unwrap();
        assert!(!profile.facts.iter().any(|f| f.contains(&key)), "segredo vazou: {:?}", profile.facts);
        assert!(profile.facts.iter().any(|f| f.contains("ada@example.com")), "PII social não deveria ser redigida aqui");
    }

    /// NEGATIVO — bloqueado por `memory.enabled=false`, nada é gravado.
    #[test]
    fn set_user_profile_blocked_by_memory_enabled_false_persists_nothing() {
        let mem = store();
        let out = set_user_profile(&mem, false, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();
        assert!(out.is_none());
        assert!(get_user_profile(&mem).unwrap().is_none());
    }

    /// NEGATIVO — `forget` sem perfil gravado é NO-OP, mesmo com a gravação bloqueada (nada a
    /// bloquear: o gate só protege uma escrita que nem ia acontecer).
    #[test]
    fn forget_without_a_profile_is_a_no_op_regardless_of_the_recording_gate() {
        let mem = store();
        assert!(matches!(forget_user_profile(&mem, false, ForgetUserProfileInput::default(), 1).unwrap(), ForgetOutcome::NothingToForget));
    }

    /// `forget(all=true)` apaga os DADOS (nenhum campo de PII sobrevive), mas o REGISTRO continua
    /// existindo — `get_user_profile` volta `Some(perfil vazio)`, nunca `None`, mesmo comportamento
    /// do TS (`upsert`, nunca `delete`).
    #[test]
    fn forget_all_wipes_the_data_but_keeps_an_empty_record() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), facts: vec!["gosta de rust".into()], ..Default::default() }, 1).unwrap();

        let outcome = forget_user_profile(&mem, true, ForgetUserProfileInput { all: true, ..Default::default() }, 2).unwrap();
        let ForgetOutcome::Forgotten(profile) = outcome else { panic!("esperava Forgotten") };
        assert_eq!(profile.preferred_name, None);
        assert!(profile.facts.is_empty());

        let reread = get_user_profile(&mem).unwrap().unwrap();
        assert_eq!(reread.preferred_name, None, "registro sobrevive, mas sem PII nenhuma");
        assert!(reread.facts.is_empty());
    }

    /// Esquecimento seletivo por campo nomeado não toca no que não foi pedido.
    #[test]
    fn forget_selective_field_drops_only_the_named_field() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), pronouns: Some("she/her".into()), ..Default::default() }, 1).unwrap();

        let outcome = forget_user_profile(&mem, true, ForgetUserProfileInput { fields: vec!["preferredName".into()], ..Default::default() }, 2).unwrap();
        let ForgetOutcome::Forgotten(profile) = outcome else { panic!("esperava Forgotten") };
        assert_eq!(profile.preferred_name, None);
        assert_eq!(profile.pronouns.as_deref(), Some("she/her"), "campo não pedido não pode sumir");
    }

    /// Esquecimento seletivo de fatos/idiomas remove só o valor citado.
    #[test]
    fn forget_selective_fact_removes_only_the_named_fact() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { facts: vec!["gosta de rust".into(), "mora em SP".into()], ..Default::default() }, 1).unwrap();

        let outcome = forget_user_profile(&mem, true, ForgetUserProfileInput { facts: vec!["gosta de rust".into()], ..Default::default() }, 2).unwrap();
        let ForgetOutcome::Forgotten(profile) = outcome else { panic!("esperava Forgotten") };
        assert_eq!(profile.facts, vec!["mora em SP".to_string()]);
    }

    /// NEGATIVO — perfil existente + `memory.enabled=false` reporta `Blocked`, nunca finge que
    /// esqueceu (a classe "resposta que significa não fiz" que este módulo tem de evitar).
    #[test]
    fn forget_of_an_existing_profile_blocked_by_memory_enabled_false_reports_blocked_not_success() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();

        let outcome = forget_user_profile(&mem, false, ForgetUserProfileInput { all: true, ..Default::default() }, 2).unwrap();
        assert!(matches!(outcome, ForgetOutcome::Blocked));
        assert_eq!(get_user_profile(&mem).unwrap().unwrap().preferred_name.as_deref(), Some("Ada"), "bloqueado não pode ter apagado nada");
    }

    #[test]
    fn set_personality_profile_merges_scalars_and_accumulates_top_interests() {
        let mem = store();
        set_personality_profile(&mem, true, PersonalityInput { communication_style: Some("casual".into()), top_interests: vec!["rust".into()], ..Default::default() }, 1).unwrap();

        let profile = set_personality_profile(&mem, true, PersonalityInput { humor_tolerance: Some("high".into()), top_interests: vec!["games".into()], ..Default::default() }, 2)
            .unwrap()
            .unwrap();
        let personality = profile.personality.unwrap();
        assert_eq!(personality.communication_style.as_deref(), Some("casual"), "segundo set_personality não pode ter apagado o campo do primeiro");
        assert_eq!(personality.humor_tolerance.as_deref(), Some("high"));
        assert_eq!(personality.top_interests, vec!["rust".to_string(), "games".to_string()]);
    }

    /// `set_personality_profile` não pode apagar o resto do perfil (nome, idiomas...) — ele só
    /// menciona `personality` no input que repassa para `set_user_profile`.
    #[test]
    fn set_personality_profile_never_touches_the_rest_of_the_profile() {
        let mem = store();
        set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();
        let profile = set_personality_profile(&mem, true, PersonalityInput { communication_style: Some("playful".into()), ..Default::default() }, 2).unwrap().unwrap();
        assert_eq!(profile.preferred_name.as_deref(), Some("Ada"));
    }

    /// Ida e volta pelo DISCO, não só em memória: fecha o `MemoryStore` (arquivo real) e reabre no
    /// mesmo caminho — mesmo padrão de `modules::memory::mod.rs::reabrir_o_mesmo_arquivo...`.
    #[test]
    fn a_set_profile_survives_closing_and_reopening_the_same_file() {
        let dir = TempDir::new();
        let path = dir.0.join("memory.db");
        {
            let mem = MemoryStore::open(&path).unwrap();
            set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), languages: vec!["pt".into()], facts: vec!["gosta de rust".into()], ..Default::default() }, 1).unwrap();
        }
        {
            let mem = MemoryStore::open(&path).unwrap();
            let profile = get_user_profile(&mem).unwrap().unwrap();
            assert_eq!(profile.preferred_name.as_deref(), Some("Ada"));
            assert_eq!(profile.languages, vec!["pt".to_string()]);
            assert_eq!(profile.facts, vec!["gosta de rust".to_string()]);
        }
    }

    /// Mesmo teste, mas para `forget(all)`: reabrir o arquivo depois de esquecer não pode
    /// "ressuscitar" nada — a escrita do wipe tem de ter chegado ao disco.
    #[test]
    fn forgetting_a_profile_survives_closing_and_reopening_the_same_file() {
        let dir = TempDir::new();
        let path = dir.0.join("memory.db");
        {
            let mem = MemoryStore::open(&path).unwrap();
            set_user_profile(&mem, true, SetUserProfileInput { preferred_name: Some("Ada".into()), ..Default::default() }, 1).unwrap();
            forget_user_profile(&mem, true, ForgetUserProfileInput { all: true, ..Default::default() }, 2).unwrap();
        }
        {
            let mem = MemoryStore::open(&path).unwrap();
            let profile = get_user_profile(&mem).unwrap().unwrap();
            assert_eq!(profile.preferred_name, None, "reabrir o arquivo não pode trazer o PII de volta");
        }
    }
}
