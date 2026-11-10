//! Porte fiel de `apps/desktop/src/main/services/voice/services/speech-text.ts` — o único ponto
//! onde texto de auto-speak vira fala: remove tags de emoção e blocos de raciocínio ANTES de
//! enfileirar (distinto de `tts::preprocessor::sanitize_text`, que roda depois, dentro de
//! `speaker::speak`, e cobre um alfabeto de marcação diferente — colchete SÓ maiúsculo — porque
//! aquele funil protege a síntese de áudio em si, não a decisão "isto já foi dito neste turno?").

use std::sync::LazyLock;

use regex::Regex;

use super::stream_sentences::strip_reasoning_spans;

static EMOTION_TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<\|[a-z][a-z0-9_-]*(?::[\d.]+)?\|>|\[[a-z][a-z0-9_-]*(?::[\d.]+)?\]")
        .expect("padrão de tag de emoção inválido")
});
static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").expect("padrão de espaço inválido"));
static ALL_WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("padrão de espaço inválido"));

/// Único ponto onde texto vira fala; remove tags de emoção e blocos de raciocínio antes do TTS.
pub fn strip_emotion_tags(text: &str) -> String {
    let without_reasoning = strip_reasoning_spans(text);
    let without_tags = EMOTION_TAG_REGEX.replace_all(&without_reasoning, "");
    MULTI_SPACE.replace_all(&without_tags, " ").trim().to_string()
}

pub fn normalize_for_compare(text: &str) -> String {
    let clean = strip_emotion_tags(text);
    ALL_WHITESPACE.replace_all(&clean, " ").trim().to_string()
}

/// Porte de `hasSpeakableText` (`audio/tts-speaker.ts`) — mesmo alfabeto Unicode usado para decidir
/// se sobrou algo falável depois da limpeza, antes de mandar para `speak_tool::run_speak`.
pub fn has_speakable_text(text: &str) -> bool {
    text.chars().any(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_uppercase_and_lowercase_angle_emotion_tags() {
        assert_eq!(strip_emotion_tags("<|HAPPY|> oi tudo bem <|SAD|>"), "oi tudo bem");
        assert_eq!(strip_emotion_tags("<|smug:0.8|> zoeira pura <|laugh|>"), "zoeira pura");
    }

    #[test]
    fn strips_bracket_emotion_tags_case_insensitively_and_with_intensity() {
        assert_eq!(strip_emotion_tags("[SMUG] belo dano [laugh]"), "belo dano");
        assert_eq!(strip_emotion_tags("[ANGRY:0.5] que isso [panic]"), "que isso");
    }

    /// Smoke 2026-09-27: body actions are hyphenated (`throw-tomato`) and "[THROW-TOMATO]" was spoken.
    #[test]
    fn strips_hyphenated_action_tags() {
        assert_eq!(strip_emotion_tags("[THROW-TOMATO] De novo? <|six-seven|> kkk"), "De novo? kkk");
    }

    #[test]
    fn preserves_clean_text_untouched() {
        assert_eq!(strip_emotion_tags("mensagem normal sem marcadores"), "mensagem normal sem marcadores");
    }

    #[test]
    fn normalizes_spaces_and_trims_properly() {
        assert_eq!(normalize_for_compare("  [HAPPY]   olá    mundo  "), "olá mundo");
    }
}
