//! Cooldown/anti-spam da extração automática de menções — porte de
//! `apps/desktop/src/features/person-catalog/handlers/person-catalog-mention-detect.ts` (pré-filtro)
//! e `handlers/person-catalog-mention-gate.ts` (arquivo inteiro).
//!
//! Puro e sem I/O de propósito: `lifecycle.rs` é o único chamador, dono de UMA instância por
//! processo (vive dentro da task assinante, nunca um `OnceLock` global) — mesma razão de
//! `person-catalog-mention-gate.ts` ser estado de módulo no TS (um `Map`/timestamps por processo,
//! não persistido).

use std::collections::HashMap;

/// = `EXTRACTION_MIN_INTERVAL_MS`.
const EXTRACTION_MIN_INTERVAL_MS: i64 = 15_000;
/// = `RECENT_TEXT_TTL_MS`.
const RECENT_TEXT_TTL_MS: i64 = 5 * 60_000;
/// = `RECENT_TEXT_MAX`.
const RECENT_TEXT_MAX: usize = 100;
/// = `PERSON_TOOL_GRACE_MS`.
const PERSON_TOOL_GRACE_MS: i64 = 30_000;

/// Porte de `shouldNotePersonTool` para o assinante real de `EventKey::ToolResult`
/// (`lifecycle.rs`). O payload publicado por `modules/chat/send.rs::turn_event_bus` é
/// `{"name", "success"}` — SEM `action`
/// (`TurnEvent::ToolCallFinished` só carrega nome e resultado booleano, não o `data.action` que a
/// tool devolve). A TS lê `action` de `result.data.action` porque o barramento TS carrega o
/// resultado inteiro da tool; o Rust barramento de chat só carrega um resumo. Sem `action`, este
/// porte degrada para "qualquer escrita bem-sucedida da tool `person_catalog`" — mais permissiva
/// que o TS (nota 30s de graça em `get`/`search`/`list` também, que são leitura, não escrita), mas
/// nunca menos segura: o pior caso é adiar uma extração por até 30s a mais, nunca perder uma
/// catalogação. Divergência nomeada, não escondida — apertar isso pede `data.action` no payload de
/// `ToolResult`, fora de `person_catalog/**`.
pub(super) fn should_note_person_tool_result(tool_name: &str, success: bool) -> bool {
    success && tool_name == "person_catalog"
}

fn normalize_text(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Porte do estado de módulo de `person-catalog-mention-gate.ts` — um `HashMap` de texto
/// normalizado -> timestamp faz o papel do `Map` de TTL do TS, podado por tamanho E por idade.
#[derive(Default)]
pub(super) struct MentionGate {
    last_extraction_at: Option<i64>,
    last_person_tool_use_at: Option<i64>,
    recent_texts: HashMap<String, i64>,
}

impl MentionGate {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Porte de `notePersonToolUsed`.
    pub(super) fn note_person_tool_used(&mut self, now: i64) {
        self.last_person_tool_use_at = Some(now);
    }

    /// Porte de `shouldExtractMention` — as 3 guardas na MESMA ordem: graça pós-escrita da tool,
    /// intervalo mínimo entre extrações, texto repetido dentro do TTL.
    /// `urgent` (a relation reference, or an open clarification being answered) skips only the
    /// minimum interval — people facts said in quick succession must not be dropped.
    pub(super) fn should_extract_mention(&mut self, text: &str, now: i64, urgent: bool) -> bool {
        if let Some(t) = self.last_person_tool_use_at {
            if now - t < PERSON_TOOL_GRACE_MS {
                return false;
            }
        }
        if let Some(t) = self.last_extraction_at.filter(|_| !urgent) {
            if now - t < EXTRACTION_MIN_INTERVAL_MS {
                return false;
            }
        }
        self.prune_recent_texts(now);
        if let Some(&seen_at) = self.recent_texts.get(&normalize_text(text)) {
            if now - seen_at < RECENT_TEXT_TTL_MS {
                return false;
            }
        }
        true
    }

    /// Porte de `beginMentionExtraction` — registrado ANTES da chamada ao LLM (mesmo comentário do
    /// TS: um provedor falhando não pode martelar a mesma frase a cada 15s).
    pub(super) fn begin_mention_extraction(&mut self, now: i64) {
        self.last_extraction_at = Some(now);
    }

    /// Porte de `commitMentionText`.
    pub(super) fn commit_mention_text(&mut self, text: &str, now: i64) {
        self.prune_recent_texts(now);
        self.recent_texts.insert(normalize_text(text), now);
        if self.recent_texts.len() > RECENT_TEXT_MAX {
            if let Some(oldest_key) = self.recent_texts.iter().min_by_key(|(_, at)| **at).map(|(k, _)| k.clone()) {
                self.recent_texts.remove(&oldest_key);
            }
        }
    }

    fn prune_recent_texts(&mut self, now: i64) {
        self.recent_texts.retain(|_, at| now - *at < RECENT_TEXT_TTL_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_note_person_tool_result_requires_success_and_the_right_tool_name() {
        assert!(should_note_person_tool_result("person_catalog", true));
        assert!(!should_note_person_tool_result("person_catalog", false), "falha não conta");
        assert!(!should_note_person_tool_result("web", true), "tool errada não conta");
    }

    #[test]
    fn extraction_is_allowed_by_default() {
        let mut gate = MentionGate::new();
        assert!(gate.should_extract_mention("oi, conhece a Ada?", 0, false));
    }

    #[test]
    fn blocks_within_the_person_tool_grace_period() {
        let mut gate = MentionGate::new();
        gate.note_person_tool_used(1_000);
        assert!(!gate.should_extract_mention("texto novo", 1_000 + PERSON_TOOL_GRACE_MS - 1, false));
        assert!(gate.should_extract_mention("texto novo", 1_000 + PERSON_TOOL_GRACE_MS, false));
    }

    #[test]
    fn blocks_within_the_minimum_interval_between_extractions() {
        let mut gate = MentionGate::new();
        gate.begin_mention_extraction(1_000);
        assert!(!gate.should_extract_mention("outro texto", 1_000 + EXTRACTION_MIN_INTERVAL_MS - 1, false));
        assert!(gate.should_extract_mention("outro texto", 1_000 + EXTRACTION_MIN_INTERVAL_MS, false));
        assert!(gate.should_extract_mention("minha esposa se chama Dani", 1_001, true), "a relation fact right after another is not dropped");
    }

    #[test]
    fn blocks_the_same_normalized_text_within_the_ttl() {
        let mut gate = MentionGate::new();
        gate.commit_mention_text("Fala com a Ada", 0);
        assert!(!gate.should_extract_mention("  fala com a ada  ", RECENT_TEXT_TTL_MS - 1, false), "mesmo texto normalizado");
        assert!(gate.should_extract_mention("fala com a ada", RECENT_TEXT_TTL_MS, false), "TTL expirou");
    }

    #[test]
    fn caps_recent_texts_at_the_configured_maximum() {
        let mut gate = MentionGate::new();
        for i in 0..(RECENT_TEXT_MAX + 10) {
            gate.commit_mention_text(&format!("texto {i}"), i as i64);
        }
        assert!(gate.recent_texts.len() <= RECENT_TEXT_MAX, "não podou o excedente: {}", gate.recent_texts.len());
    }
}
