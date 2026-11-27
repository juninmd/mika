//! Que provedor cobra o quê, e se ele expõe saldo por API. Fonte única para as telas de provider
//! (LLM, TTS, STT): os ids são exatamente os que cada tela salva em `llm.provider`/`voice.*Provider`.

/// Unidade do uso que a Mika mede localmente para o provedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Meter {
    Tokens,
    Characters,
    Seconds,
}

impl Meter {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Characters => "characters",
            Self::Seconds => "seconds",
        }
    }
}

/// API oficial de saldo/cota do provedor. `None` = o provedor não publica saldo para a chave comum
/// (Google AI, Groq, Azure, Google TTS; a OpenAI só com chave de admin) — a tela diz isso em vez de
/// inventar um número.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BalanceApi {
    OpenRouter,
    ElevenLabs,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Spec {
    /// Roda na máquina (Ollama, Unsloth, Supertonic, Whisper local): sem conta, sem cobrança.
    pub local: bool,
    /// Serviço gratuito sem conta própria da Mika (Web Speech do navegador, voz do sistema).
    pub free: bool,
    pub meter: Option<Meter>,
    pub api: BalanceApi,
}

const fn paid(meter: Meter, api: BalanceApi) -> Spec {
    Spec { local: false, free: false, meter: Some(meter), api }
}

const fn local(meter: Option<Meter>) -> Spec {
    Spec { local: true, free: true, meter, api: BalanceApi::None }
}

const FREE: Spec = Spec { local: false, free: true, meter: None, api: BalanceApi::None };

pub(super) fn spec(provider: &str) -> Option<Spec> {
    use BalanceApi as A;
    use Meter as M;
    Some(match provider {
        "openrouter" => paid(M::Tokens, A::OpenRouter),
        "google" | "openaiCompatible" => paid(M::Tokens, A::None),
        "ollama" | "unsloth" => local(Some(M::Tokens)),
        "elevenlabs" => paid(M::Characters, A::ElevenLabs),
        "azure-tts" | "google-tts" | "openai-tts" => paid(M::Characters, A::None),
        "supertonic" | "piper" => local(None),
        "groq-whisper" | "openai-whisper" => paid(M::Seconds, A::None),
        "nemotron" | "parakeet" | "electron" | "local-whisper" | "local" => local(None),
        "browser" | "system" => FREE,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os ids que as telas salvam (LLM `PROVIDERS`, TTS `TTSProvider`, STT `sttProviderChoices`):
    /// um id novo numa tela sem entrada aqui deixaria o card de saldo mudo.
    #[test]
    fn every_provider_id_offered_by_a_settings_screen_is_known() {
        let llm = include_str!("../../../../src/renderer/settings/llm/llm-providers.ts");
        let tts = include_str!("../../../../src/renderer/settings/voice/tts-provider-constants.ts");
        let stt = include_str!("../../../../src/renderer/settings/voice/stt-provider-choices.ts");
        for id in ["google", "openrouter", "ollama", "openaiCompatible", "unsloth"] {
            assert!(llm.contains(&format!("id: '{id}'")) && spec(id).is_some(), "{id}");
        }
        for id in ["elevenlabs", "azure-tts", "google-tts", "openai-tts", "piper", "supertonic", "system"] {
            assert!(tts.contains(&format!("'{id}'")) && spec(id).is_some(), "{id}");
        }
        for id in ["nemotron", "parakeet", "electron", "browser", "groq-whisper", "openai-whisper"] {
            assert!(stt.contains(&format!("id: '{id}'")) && spec(id).is_some(), "{id}");
        }
        assert_eq!(spec("inventado"), None);
    }

    #[test]
    fn only_openrouter_and_elevenlabs_publish_a_balance_for_a_regular_key() {
        let with_api: Vec<_> = ["openrouter", "google", "openaiCompatible", "elevenlabs", "azure-tts", "google-tts", "openai-tts", "groq-whisper", "openai-whisper"]
            .into_iter()
            .filter(|id| spec(id).unwrap().api != BalanceApi::None)
            .collect();
        assert_eq!(with_api, ["openrouter", "elevenlabs"]);
    }

    #[test]
    fn local_engines_never_claim_a_bill() {
        for id in ["ollama", "unsloth", "supertonic", "piper", "nemotron", "parakeet", "electron"] {
            let s = spec(id).unwrap();
            assert!(s.local && s.free && s.api == BalanceApi::None, "{id}");
        }
    }
}
