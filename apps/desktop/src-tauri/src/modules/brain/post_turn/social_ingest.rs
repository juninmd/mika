//! Metade REGEX de `ingestUserMessage` (`brain-social-context.ts`) — a outra metade (extração via
//! LLM) mora em [`super::facts`]; as duas alimentam
//! [`crate::modules::memory::social::SocialMemoryService`], mesmo par que `pipeline.ts::run`
//! dispara junto a cada turno.
//!
//! **`addEvent` não portado**: `ingestUserMessage` também chama `socialMemory.addEvent(text)`
//! quando acha nome/relação — `SocialMemoryService` em Rust já documenta `addEvent` como "sem
//! chamador degradando nomeado nesta rodada" (ver cabeçalho de `memory::social::mod`), e abrir a
//! tabela de eventos só para este único chamador está fora do pedido desta rodada (fatos + ingestão
//! social, não o histórico de eventos sociais). [`ingest_social_message`] cobre
//! `setUserProfile`/`addRelationship` (os dois métodos que JÁ existem em Rust), não o terceiro.
//!
//! Puramente local, sem LLM: testável direto com um `MemoryStore` em memória, sem `AppHandle`.

use regex::Regex;

use crate::modules::memory::social::{AddRelationshipInput, SetUserProfileInput, SocialMemoryService};
use crate::modules::memory::MemoryStore;

/// Um nome próprio: até 3 palavras, TODAS começando em maiúscula. O padrão antigo capturava
/// `[letras espaço]{1,40}` depois da primeira maiúscula (e o `(?i)` global fazia `\p{Lu}` casar
/// minúscula): "me chamo Ana e moro em Recife" virava o nome "Ana e moro em Recife".
const NAME: &str = r"\p{Lu}[\p{L}\d'-]*(?: \p{Lu}[\p{L}\d'-]*){0,2}";

fn name_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(&format!(r"\b(?i:meu nome é|me chamo|pode me chamar de)\s+({NAME})")).unwrap())
}

fn relationship_patterns() -> &'static [Regex] {
    static PATTERNS: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(&format!(r"\b({NAME})\s+(?:é|eh)\s+(?:meu|minha)\s+([\p{{L}}\d-]{{3,24}})")).unwrap(),
            Regex::new(&format!(r"\b(?:meu|minha)\s+([\p{{L}}\d-]{{3,24}})\s+(?:se chama|chama|é)\s+({NAME})")).unwrap(),
        ]
    })
}

/// Porte da metade regex de `ingestUserMessage`: nome preferido (`setUserProfile`) e relações
/// (`addRelationship`) — ver o cabeçalho para o que ficou de fora.
pub(super) fn ingest_social_message(mem: &MemoryStore, recording_allowed: bool, text: &str, now_ms: i64) {
    if let Some(captures) = name_pattern().captures(text) {
        if let Some(name) = captures.get(1) {
            let input = SetUserProfileInput { preferred_name: Some(name.as_str().trim().to_string()), ..Default::default() };
            let svc = SocialMemoryService::new(mem);
            if let Err(error) = svc.set_user_profile(recording_allowed, input, now_ms) {
                log::warn!("ingestão social: falha ao gravar nome preferido: {error}");
            }
        }
    }

    for pattern in relationship_patterns() {
        for captures in pattern.captures_iter(text) {
            let first_is_name = captures.get(1).map(|m| m.as_str().chars().next().is_some_and(char::is_uppercase)).unwrap_or(false);
            let (name, label) = if first_is_name { (captures.get(1), captures.get(2)) } else { (captures.get(2), captures.get(1)) };
            let (Some(name), Some(label)) = (name, label) else { continue };

            let svc = SocialMemoryService::new(mem);
            let input = AddRelationshipInput { name: name.as_str().trim().to_string(), label: label.as_str().trim().to_string(), confidence: None };
            if let Err(error) = svc.add_relationship(recording_allowed, input, now_ms) {
                log::warn!("ingestão social: falha ao gravar relação: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn ingest_social_message_grava_o_nome_preferido() {
        let mem = store();
        ingest_social_message(&mem, true, "oi, meu nome é Junin", 1);

        let svc = SocialMemoryService::new(&mem);
        let profile = svc.get_user_profile().unwrap().unwrap();
        assert_eq!(profile.preferred_name.as_deref(), Some("Junin"));
    }

    #[test]
    fn ingest_social_message_grava_relacao_forma_pessoa_e_meu_rotulo() {
        let mem = store();
        ingest_social_message(&mem, true, "Dani é minha esposa", 1);

        let svc = SocialMemoryService::new(&mem);
        let graph = svc.get_graph().unwrap();
        assert_eq!(graph.people.len(), 1);
        assert_eq!(graph.people[0].primary_name, "Dani");
    }

    #[test]
    fn ingest_social_message_grava_relacao_forma_meu_rotulo_se_chama_pessoa() {
        let mem = store();
        ingest_social_message(&mem, true, "minha esposa se chama Dani", 1);

        let svc = SocialMemoryService::new(&mem);
        let graph = svc.get_graph().unwrap();
        assert_eq!(graph.people.len(), 1);
        assert_eq!(graph.people[0].primary_name, "Dani");
    }

    #[test]
    fn ingest_social_message_nao_grava_nada_sem_padrao_reconhecido() {
        let mem = store();
        ingest_social_message(&mem, true, "hoje o jogo foi bom", 1);

        let svc = SocialMemoryService::new(&mem);
        assert!(svc.get_user_profile().unwrap().is_none());
        assert!(svc.get_graph().unwrap().people.is_empty());
    }

    #[test]
    fn ingest_social_message_respeita_recording_allowed_false() {
        let mem = store();
        ingest_social_message(&mem, false, "meu nome é Junin", 1);

        let svc = SocialMemoryService::new(&mem);
        assert!(svc.get_user_profile().unwrap().is_none());
    }

    #[test]
    fn the_name_stops_at_the_first_lowercase_word() {
        let mem = store();
        ingest_social_message(&mem, true, "me chamo Ana e moro em Recife", 1);
        let profile = SocialMemoryService::new(&mem).get_user_profile().unwrap().unwrap();
        assert_eq!(profile.preferred_name.as_deref(), Some("Ana"), "antes: \"Ana e moro em Recife\"");
    }

    #[test]
    fn a_capitalised_sentence_opener_is_not_part_of_the_name() {
        let mem = store();
        ingest_social_message(&mem, true, "Ontem o Bruno é meu colega", 1);
        let people = SocialMemoryService::new(&mem).get_graph().unwrap().people;
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].primary_name, "Bruno", "antes: \"Ontem o Bruno\"");
    }

    #[test]
    fn a_plain_and_is_not_a_copula() {
        let mem = store();
        ingest_social_message(&mem, true, "minha mãe e Pedro foram ao mercado", 1);
        assert!(SocialMemoryService::new(&mem).get_graph().unwrap().people.is_empty(), "antes: criava \"Pedro foram ao mercado\"");
    }

    #[test]
    fn a_two_word_name_is_kept_whole() {
        let mem = store();
        ingest_social_message(&mem, true, "Maria Clara é minha prima", 1);
        let people = SocialMemoryService::new(&mem).get_graph().unwrap().people;
        assert_eq!(people[0].primary_name, "Maria Clara");
    }
}
