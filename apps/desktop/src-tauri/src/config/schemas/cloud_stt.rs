//! Catálogo dos STT em nuvem — fonte ÚNICA de endpoint e modelos. A lista de modelos vivia duas
//! vezes (o schema que desenha o select e `stt_cloud::models`, que valida a escolha), e a URL de
//! `/models` três (STT, botão "Testar" de credenciais e o próprio STT de novo): acrescentar um
//! modelo num lado e esquecer o outro fazia o select oferecer um id que a transcrição recusava.
//!
//! Conferido em 2026-09-28: Groq só serve os dois Whisper (console.groq.com/docs/speech-to-text); a
//! OpenAI recomenda `gpt-transcribe` e remove `whisper-1`/`gpt-4o*-transcribe` em 2027-02-26
//! (developers.openai.com/api/docs/deprecations) — ficam na lista até lá para quem já os usa.

use crate::config::schema::UiOption;

pub(crate) const GROQ_TRANSCRIPTIONS_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
pub(crate) const OPENAI_TRANSCRIPTIONS_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
/// `GET /models` valida a chave sem gastar cota de transcrição (STT e botão "Testar" de credenciais).
pub(crate) const GROQ_MODELS_URL: &str = "https://api.groq.com/openai/v1/models";
pub(crate) const OPENAI_MODELS_URL: &str = "https://api.openai.com/v1/models";

/// O 1º de cada lista é o default.
pub(crate) const GROQ_MODELS: &[&str] = &["whisper-large-v3-turbo", "whisper-large-v3"];
pub(crate) const OPENAI_MODELS: &[&str] = &["gpt-transcribe", "gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"];

/// O rótulo do select é o próprio id do modelo.
const fn same(id: &'static str) -> UiOption {
    UiOption { label: id, value: id }
}

pub(crate) const GROQ_OPTIONS: &[UiOption] = &[same(GROQ_MODELS[0]), same(GROQ_MODELS[1])];
pub(crate) const OPENAI_OPTIONS: &[UiOption] =
    &[same(OPENAI_MODELS[0]), same(OPENAI_MODELS[1]), same(OPENAI_MODELS[2]), same(OPENAI_MODELS[3])];

#[cfg(test)]
mod tests {
    use super::*;

    /// O select e a validação não podem divergir: cada modelo aceito tem exatamente uma opção.
    #[test]
    fn select_options_cover_every_accepted_model_in_order() {
        for (models, options) in [(GROQ_MODELS, GROQ_OPTIONS), (OPENAI_MODELS, OPENAI_OPTIONS)] {
            let values: Vec<&str> = options.iter().map(|o| o.value).collect();
            assert_eq!(values, models);
        }
    }
}
