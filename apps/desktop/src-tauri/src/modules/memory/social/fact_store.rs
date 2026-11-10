//! Porte de `apps/desktop/src/main/services/memory/social/fact-store.ts`.

use crate::modules::memory::MemoryStore;

use super::fact_rules::{contradicts, fact_content_hash, fact_similarity, fact_tokens, functional_conflict};
use super::rules::{normalize_text, USER_SUBJECT};
use super::store;
use super::types::{LearnedFact, SOCIAL_TABLE_FACTS};

use crate::modules::memory::redact::redact_for_storage;

/// = `SOCIAL_SCAN_LIMIT`: teto de leitura para `addFact`; sem isso ele cresce O(n²) com o histórico
/// de vida do app.
pub const SOCIAL_SCAN_LIMIT: i64 = 300;

/// = os campos de entrada de `SocialMemoryService.addFact`.
#[derive(Default)]
pub struct AddFactInput {
    pub subject: Option<String>,
    pub fact: String,
    pub confidence: Option<f64>,
    pub source_turn_id: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
}

/// = `addFact`: extrai o fato, dedup/funde contra o histórico do subject — contradiz -> supersede o
/// antigo, repete -> reforça a confiança do existente, senão -> grava como fato novo.
///
/// **Redação seletiva**: [`redact_secrets`] tira segredo (API key, token) do texto ANTES de gravar,
/// mas preserva PII social (email, telefone) — é o propósito desta memória, mesma nota do TS.
pub fn add_fact(mem: &MemoryStore, input: AddFactInput, now: i64) -> Result<LearnedFact, String> {
    let clean = redact_for_storage(input.fact.trim());
    let subject = input.subject.unwrap_or_else(|| USER_SUBJECT.to_string());
    let new_conf = input.confidence.unwrap_or(0.8);

    let existing: Vec<LearnedFact> = store::get_recent_by_subject(mem, SOCIAL_TABLE_FACTS, &subject, SOCIAL_SCAN_LIMIT)?;
    let candidates: Vec<&LearnedFact> = existing.iter().filter(|f| f.active != Some(false)).collect();

    let id = format!("{}:{}", normalize_text(&subject), fact_content_hash(&normalize_text(&clean)));
    let fact = LearnedFact {
        id: id.clone(),
        subject: subject.clone(),
        fact: clean.clone(),
        confidence: new_conf,
        predicate: input.predicate.clone(),
        object: input.object.clone(),
        active: Some(true),
        superseded_by: None,
        source_turn_id: input.source_turn_id.clone(),
        created_at: Some(now),
        updated_at: now,
    };

    let superseded: Vec<&LearnedFact> = candidates
        .iter()
        .filter(|f| {
            f.id != fact.id
                && (contradicts(&f.fact, &clean)
                    || functional_conflict(f.predicate.as_deref(), f.object.as_deref(), input.predicate.as_deref(), input.object.as_deref()))
        })
        .copied()
        .collect();
    if !superseded.is_empty() {
        store::upsert(mem, SOCIAL_TABLE_FACTS, &fact.id, now as f64, &fact)?;
        for old in superseded {
            let retired = LearnedFact { active: Some(false), superseded_by: Some(fact.id.clone()), updated_at: now, ..old.clone() };
            store::upsert(mem, SOCIAL_TABLE_FACTS, &retired.id, now as f64, &retired)?;
        }
        return Ok(fact);
    }

    let clean_tokens = fact_tokens(&clean);
    if let Some(duplicate) = candidates.iter().find(|f| fact_similarity(&f.fact, &clean_tokens) >= 0.75) {
        let reinforced = LearnedFact {
            fact: clean,
            confidence: ((duplicate.confidence + new_conf) / 2.0 + 0.05).min(1.0),
            active: Some(true),
            source_turn_id: input.source_turn_id.or_else(|| duplicate.source_turn_id.clone()),
            predicate: input.predicate.or_else(|| duplicate.predicate.clone()),
            object: input.object.or_else(|| duplicate.object.clone()),
            created_at: duplicate.created_at.or(Some(duplicate.updated_at)),
            updated_at: now,
            ..(*duplicate).clone()
        };
        store::upsert(mem, SOCIAL_TABLE_FACTS, &reinforced.id, now as f64, &reinforced)?;
        return Ok(reinforced);
    }

    store::upsert(mem, SOCIAL_TABLE_FACTS, &fact.id, now as f64, &fact)?;
    Ok(fact)
}

/// Edição de um fato pelo usuário. Texto vazio é recusado, o texto passa por [`redact_secrets`] e a
/// confiança é limitada a 1.0. O id é derivado do TEXTO, então o fato editado ganha id novo e o
/// antigo sai: se a extração reaprender depois o texto original, ela cria um fato novo em vez de
/// sobrescrever, sem perceber, a edição do usuário. `predicate`/`object` velhos ficam para trás —
/// descreviam o texto antigo. `Ok(None)`: o id não existe.
pub fn update_fact(mem: &MemoryStore, id: &str, text: &str, confidence: Option<f64>, now: i64) -> Result<Option<LearnedFact>, String> {
    let Some(existing) = store::get_by_id::<LearnedFact>(mem, SOCIAL_TABLE_FACTS, id)? else {
        return Ok(None);
    };
    let clean = redact_for_storage(text.trim());
    if clean.is_empty() {
        return Err("fact is required".to_string());
    }
    let new_id = format!("{}:{}", normalize_text(&existing.subject), fact_content_hash(&normalize_text(&clean)));
    let text_changed = clean != existing.fact;
    let updated = LearnedFact {
        id: new_id.clone(),
        fact: clean,
        confidence: confidence.unwrap_or(existing.confidence).clamp(0.0, 1.0),
        predicate: if text_changed { None } else { existing.predicate.clone() },
        object: if text_changed { None } else { existing.object.clone() },
        active: Some(true),
        superseded_by: None,
        updated_at: now,
        ..existing.clone()
    };
    store::upsert(mem, SOCIAL_TABLE_FACTS, &updated.id, now as f64, &updated)?;
    if new_id != id {
        store::delete(mem, SOCIAL_TABLE_FACTS, id)?;
    }
    Ok(Some(updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(fact: &str) -> AddFactInput {
        AddFactInput { fact: fact.to_string(), ..Default::default() }
    }

    fn store_kv() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    /// Longo prazo de verdade é DISCO: fecha o `MemoryStore` de arquivo e reabre no mesmo caminho.
    /// A correção também tem de chegar ao disco — reabrir não pode ressuscitar o fato superado.
    #[test]
    fn a_learned_fact_and_its_correction_survive_closing_and_reopening_the_same_file() {
        let dir = std::env::temp_dir().join(format!("mika-fact-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.db");
        {
            let mem = MemoryStore::open(&path).unwrap();
            add_fact(&mem, input("gosta de café"), 1).unwrap();
            add_fact(&mem, input("não gosta mais de café, prefere chá"), 2).unwrap();
        }
        let reopened = {
            let mem = MemoryStore::open(&path).unwrap();
            store::get_recent_by_subject::<LearnedFact>(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap()
        };
        let _ = std::fs::remove_dir_all(&dir);

        let active: Vec<&str> = reopened.iter().filter(|f| f.active != Some(false)).map(|f| f.fact.as_str()).collect();
        assert_eq!(active, vec!["não gosta mais de café, prefere chá"], "só a correção segue ativa depois de reabrir");
        assert_eq!(reopened.len(), 2, "o fato superado continua retido no disco, inativo");
    }

    #[test]
    fn add_fact_writes_a_new_fact_and_reads_it_back() {
        let mem = store_kv();
        let fact = add_fact(&mem, input("gosta de rust"), 1).unwrap();
        assert_eq!(fact.subject, USER_SUBJECT, "subject padrão sem `input.subject` é o canônico pt-BR, não o \"user\" em inglês");
        assert_eq!(fact.fact, "gosta de rust");

        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn add_fact_redacts_a_secret_before_it_ever_touches_the_store() {
        let mem = store_kv();
        let key = format!("sk-ant-{}", "a".repeat(40));
        let fact = add_fact(&mem, input(&format!("usa a chave {key} no cliente")), 1).unwrap();
        assert!(!fact.fact.contains(&key), "segredo vazou: {}", fact.fact);
    }

    #[test]
    fn add_fact_keeps_social_pii_because_that_is_the_point_of_this_memory() {
        let mem = store_kv();
        let fact = add_fact(&mem, input("email dela e ada@example.com"), 1).unwrap();
        assert!(fact.fact.contains("ada@example.com"), "PII social não deveria ser redigida aqui");
    }

    #[test]
    fn add_fact_reinforces_a_near_duplicate_instead_of_creating_a_second_row() {
        let mem = store_kv();
        add_fact(&mem, AddFactInput { fact: "gosta de programar em rust".into(), confidence: Some(0.6), ..Default::default() }, 1).unwrap();
        add_fact(&mem, AddFactInput { fact: "gosta muito de programar em rust".into(), confidence: Some(0.9), ..Default::default() }, 2).unwrap();

        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 1, "quase-duplicata reforça, não duplica");
        assert!(all[0].confidence > 0.6, "confiança devia subir com o reforço");
    }

    #[test]
    fn add_fact_supersedes_a_contradicted_fact_instead_of_deleting_it() {
        let mem = store_kv();
        add_fact(&mem, input("fuma cigarro"), 1).unwrap();
        let newer = add_fact(&mem, input("parou de fumar cigarro"), 2).unwrap();

        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 2, "o fato antigo fica retido, marcado inativo — nunca é apagado");
        let old = all.iter().find(|f| f.id != newer.id).unwrap();
        assert_eq!(old.active, Some(false));
        assert_eq!(old.superseded_by.as_deref(), Some(newer.id.as_str()));
    }

    /// O caso de uso do brief, literal: uma correção falada em pt-BR ("não, prefiro X") tem de
    /// SUPERSEDER o fato antigo, não empilhar dois fatos ativos contraditórios. Usa o subject padrão
    /// (sem `input.subject`) de propósito — é exatamente o caminho que a extração automática
    /// (`post_turn::facts::run`) usa quando o modelo não informa `subject` (task item 5).
    #[test]
    fn a_realistic_pt_br_correction_supersedes_the_earlier_fact_end_to_end() {
        let mem = store_kv();
        let original = add_fact(&mem, input("gosta de café"), 1).unwrap();

        // "Não, eu prefiro chá" — a MESMA frase que o brief cita como exemplo de correção.
        let corrected = add_fact(&mem, input("não gosta mais de café, prefere chá"), 2).unwrap();
        assert_ne!(original.id, corrected.id, "correção contraditória grava um fato NOVO, não reforça o antigo");

        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 2, "o fato antigo fica retido, marcado inativo — correção nunca apaga histórico");

        let old = all.iter().find(|f| f.id == original.id).unwrap();
        assert_eq!(old.active, Some(false), "o fato sobre café tem de ficar inativo depois da correção");
        assert_eq!(old.superseded_by.as_deref(), Some(corrected.id.as_str()));

        let new = all.iter().find(|f| f.id == corrected.id).unwrap();
        assert_eq!(new.active, Some(true));
        assert_eq!(new.subject, USER_SUBJECT, "os dois lados da correção caem no MESMO subject canônico");
    }

    #[test]
    fn add_fact_scopes_similarity_and_contradiction_to_the_same_subject_only() {
        let mem = store_kv();
        add_fact(&mem, AddFactInput { subject: Some("Ada".into()), fact: "fuma cigarro".into(), ..Default::default() }, 1).unwrap();
        add_fact(&mem, AddFactInput { subject: Some("Bea".into()), fact: "parou de fumar cigarro".into(), ..Default::default() }, 2).unwrap();

        let ada: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, "Ada", 10).unwrap();
        assert_eq!(ada.len(), 1, "fato de outro subject não pode superseder este");
        assert_eq!(ada[0].active, Some(true));
    }

    /// Trava a paridade entre esta cópia (núcleo) e `person_catalog::pii::redact_secrets`
    /// (feature) — os 4 padrões têm de continuar idênticos nos dois lados. Só o teste cruza a
    /// fronteira núcleo/feature; o código de produção nunca faz isso (ver o cabeçalho de
    /// [`redact_secrets`]).
    #[test]
    fn redact_secrets_matches_the_person_catalog_copy_pattern_for_pattern() {
        use crate::modules::features::person_catalog::pii::redact_secrets as feature_redact_secrets;
        use crate::modules::memory::redact::redact_secrets;

        let key = format!("sk-ant-{}", "a".repeat(40));
        let token = format!("ghp_{}", "b".repeat(36));
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dQw4w9WgXcQ_dGVzdHNpZ25hdHVyZQ";
        let samples = [
            format!("chave {key} no cliente"),
            format!("token: {token}"),
            format!("token={jwt}"),
            "apiKey=abcdef1234567890".to_string(),
            "email ada@example.com, cpf 123.456.789-01".to_string(),
            "texto sem nenhum segredo".to_string(),
        ];
        for sample in samples {
            assert_eq!(redact_secrets(&sample), feature_redact_secrets(&sample), "divergiu para {sample:?}");
        }
    }

    #[test]
    fn two_facts_that_differ_only_in_a_short_word_do_not_overwrite_each_other() {
        let mem = store_kv();
        add_fact(&mem, input("joga CS"), 1).unwrap();
        add_fact(&mem, input("joga R6"), 2).unwrap();
        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn non_latin_facts_are_stored_separately() {
        let mem = store_kv();
        add_fact(&mem, input("고양이를 좋아해요"), 1).unwrap();
        add_fact(&mem, input("매운 음식을 못 먹어요"), 2).unwrap();
        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 2, "cada fato em coreano tem o próprio id");
    }

    #[test]
    fn a_new_residence_supersedes_the_old_one_through_predicate_and_object() {
        let mem = store_kv();
        let old = add_fact(&mem, AddFactInput { fact: "mora em São Paulo".into(), predicate: Some("mora em".into()), object: Some("São Paulo".into()), ..Default::default() }, 1).unwrap();
        let new = add_fact(&mem, AddFactInput { fact: "mora em Recife".into(), predicate: Some("mora em".into()), object: Some("Recife".into()), ..Default::default() }, 2).unwrap();
        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        let retired = all.iter().find(|f| f.id == old.id).unwrap();
        assert_eq!(retired.active, Some(false));
        assert_eq!(retired.superseded_by.as_deref(), Some(new.id.as_str()));
    }

    #[test]
    fn update_fact_redacts_revalidates_and_gets_a_new_id_so_a_relearned_original_cannot_overwrite_the_edit() {
        let mem = store_kv();
        let original = add_fact(&mem, input("gosta de rust"), 1).unwrap();

        assert!(update_fact(&mem, &original.id, "   ", None, 2).is_err(), "texto vazio é recusado");
        let key = format!("sk-ant-{}", "a".repeat(40));
        let edited = update_fact(&mem, &original.id, &format!("usa a chave {key}"), Some(1.7), 3).unwrap().unwrap();
        assert!(!edited.fact.contains(&key), "a edição passa pela mesma redação de segredos");
        assert_eq!(edited.confidence, 1.0, "confiança é limitada a 1.0");
        assert_ne!(edited.id, original.id, "o id acompanha o texto novo");

        let all: Vec<LearnedFact> = store::get_recent_by_subject(&mem, SOCIAL_TABLE_FACTS, USER_SUBJECT, 10).unwrap();
        assert_eq!(all.len(), 1, "o fato antigo sai, a edição fica");
        assert!(update_fact(&mem, "nao-existe", "qualquer coisa aqui", None, 4).unwrap().is_none());
    }
}
