//! Namespace de configuração do módulo `voice`, portado dos `*.schema.ts` correspondentes:
//! `voice.schema.ts`, `voice-config.schema.ts`, `voice-runtime-config.schema.ts`,
//! `voice-cloud-tts-config.schema.ts`, `voice-local-tts-config.schema.ts`,
//! `voice-stt-config.schema.ts`, `voice-elevenlabs-config.schema.ts`,
//! `voice-provider-catalog.schema.ts` e `voice-provider-fields.schema.ts`
//! (`apps/desktop/src/main/services/voice/`).
//!
//! Todos os grupos (TTS runtime, cada provedor de TTS e o ASR/STT) usam `prefix: ConfigKey.Voice`
//! em `defineFeatureSchema`, então viram um único namespace `voice` aqui — a mesma divisão em
//! sub-funções dos arquivos de origem existe só para a revisão de paridade ficar rastreável.

use super::cloud_stt;
use super::super::registry::Namespace;
use super::super::schema::{boolean, choice, float_range, int_range, text, FieldSchema, FieldUi, UiOption};
use super::voice_voices::{AZURE_VOICES, GOOGLE_VOICES};

/// `voice-runtime-config.schema.ts`
fn runtime_fields() -> Vec<FieldSchema> {
    vec![
        // `kokoro` FICA DE FORA das opções (o provedor está cortado, `dispatch.rs::CUT_PROVIDERS`)
        // mas continua um valor ACEITO pelo enum: um `ttsProvider` salvo antes deste corte não pode
        // virar erro de validação ao reabrir o app — `ConfigStore::open` nunca chama `validate` na
        // carga (só em `set`), então o valor antigo só fica sem opção destacada na UI, nunca crasha.
        choice(
            "ttsProvider",
            "supertonic",
            &[
                "system",
                "elevenlabs",
                "openai-tts",
                "azure-tts",
                "google-tts",
                "piper",
                "kokoro",
                "supertonic",
            ],
        )
        .with_ui(
            FieldUi::new("settings.voiceTts.field.ttsProvider.label")
                .control("select")
                .helper("settings.voiceTts.field.ttsProvider.helperText")
                .options(&[
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.supertonic",
                        value: "supertonic",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.piper",
                        value: "piper",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.elevenlabs",
                        value: "elevenlabs",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.azure",
                        value: "azure-tts",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.google",
                        value: "google-tts",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.openai",
                        value: "openai-tts",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.ttsProvider.option.system",
                        value: "system",
                    },
                ]),
        ),
        // `.describe('chave')` sem `m({...})` no original: `parseDescribeMeta` do lado TS usa a
        // mesma string como label E description — não é um esquecimento do port.
        boolean("ttsEnabled", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsEnabled").described("settings.voiceTts.field.ttsEnabled"),
        ),
        float_range("volume", 1.0, 0.0, 1.0).with_ui(
            FieldUi::new("settings.voiceTts.field.volume.label")
                .control("range")
                .helper("settings.voiceTts.field.volume.helperText"),
        ),
        boolean("autoSpeak", true)
            .with_ui(FieldUi::new("settings.voiceTts.field.autoSpeak").described("settings.voiceTts.field.autoSpeak")),
        boolean("autoMuteDuringTts", false).with_ui(
            FieldUi::new("settings.voiceTts.field.autoMuteDuringTts")
                .described("settings.voiceTts.field.autoMuteDuringTts"),
        ),
        boolean("interruptionEnabled", true).with_ui(FieldUi::new("settings.voiceTts.field.interruptionEnabled")),
        float_range("interruptionThreshold", 0.35, 0.0, 1.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.interruptionThreshold.label").control("range")),
        int_range("interruptionTriggerStreak", 3, 1, 12)
            .with_ui(FieldUi::new("settings.voiceTts.field.interruptionTriggerStreak.label").control("range")),
        int_range("interruptionCooldownMs", 1200, 250, 10000)
            .with_ui(FieldUi::new("settings.voiceTts.field.interruptionCooldownMs.label").control("range")),
        boolean("vadEndpointingEnabled", true).with_ui(FieldUi::new("settings.voiceTts.field.vadEndpointingEnabled")),
        // 1200, not 1500: this now also feeds the renderer's main-listening-path hangover
        // (speech-transcribe.ts's old hardcoded SILENCE_HANGOVER_MS), which already ran at 1200ms —
        // keeping the schema default at 1500 would have silently lengthened every user's first
        // utterance the moment the dead control got wired live.
        int_range("vadSilenceMs", 1200, 500, 5000).with_ui(
            FieldUi::new("settings.voiceTts.field.vadSilenceMs.label")
                .control("range")
                .helper("settings.voiceTts.field.vadSilenceMs.helperText"),
        ),
        float_range("vadSpeechThreshold", 0.35, 0.1, 0.9).with_ui(
            FieldUi::new("settings.voiceTts.field.vadSpeechThreshold.label")
                .control("range")
                .helper("settings.voiceTts.field.vadSpeechThreshold.helperText"),
        ),
        boolean("ttsIgnoreBrackets", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsIgnoreBrackets")
                .described("settings.voiceTts.field.ttsIgnoreBrackets"),
        ),
        boolean("ttsIgnoreParentheses", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsIgnoreParentheses")
                .described("settings.voiceTts.field.ttsIgnoreParentheses"),
        ),
        boolean("ttsIgnoreAsterisks", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsIgnoreAsterisks")
                .described("settings.voiceTts.field.ttsIgnoreAsterisks"),
        ),
        boolean("ttsIgnoreAngleBrackets", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsIgnoreAngleBrackets")
                .described("settings.voiceTts.field.ttsIgnoreAngleBrackets"),
        ),
        boolean("ttsRemoveEmoji", true).with_ui(
            FieldUi::new("settings.voiceTts.field.ttsRemoveEmoji").described("settings.voiceTts.field.ttsRemoveEmoji"),
        ),
    ]
}

/// `voice-elevenlabs-config.schema.ts`
fn elevenlabs_fields() -> Vec<FieldSchema> {
    vec![
        // A CREDENCIAL NÃO MORA AQUI. O Zod declara o campo no grupo prefixado por
        // `ConfigKey.Voice`, mas o `m({ configPath: 'apiKeys.elevenLabs' })` reaponta o ARMAZENAMENTO
        // para o namespace `apiKeys`, compartilhado entre provedores — e é o caminho de escrita, não
        // o de declaração, que o store tem de reproduzir. O port original repetiu a chave em
        // `voice.elevenLabsApiKey`: a UI gravaria num caminho que leitor nenhum consulta, e a chave
        // que o usuário já tem em `apiKeys.elevenLabs` ficaria invisível. `apiKeys.elevenLabs` é
        // declarado em `llm.rs` (o namespace não existia quando este arquivo foi escrito).
        //
        // O painel continua sendo desenhado pelo TypeScript, que mantém o controle sob voiceTts —
        // o schema Rust responde por validação e armazenamento, e é só isso que muda aqui.
        text("elevenLabsVoiceId", "EXAVITQu4vr4xnSDxMaL")
            .with_ui(FieldUi::new("settings.voiceTts.field.elevenLabsVoiceId.label")),
        choice(
            "elevenLabsModel",
            "eleven_multilingual_v2",
            &["eleven_flash_v2_5", "eleven_turbo_v2_5", "eleven_multilingual_v2"],
        )
        .with_ui(
            FieldUi::new("settings.voiceTts.field.elevenLabsModel.label")
                .control("select")
                .options(&[
                    UiOption {
                        label: "settings.voiceTts.field.elevenLabsModel.option.flash",
                        value: "eleven_flash_v2_5",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.elevenLabsModel.option.turbo",
                        value: "eleven_turbo_v2_5",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.elevenLabsModel.option.multilingual",
                        value: "eleven_multilingual_v2",
                    },
                ]),
        ),
        float_range("ttsStability", 0.5, 0.0, 1.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.ttsStability.label").control("range")),
        float_range("elevenLabsSimilarityBoost", 0.75, 0.0, 1.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.elevenLabsSimilarityBoost.label").control("range")),
        float_range("elevenLabsStyle", 0.0, 0.0, 1.0).with_ui(
            FieldUi::new("settings.voiceTts.field.elevenLabsStyle.label")
                .control("range")
                .helper("settings.voiceTts.field.elevenLabsStyle.helperText"),
        ),
        float_range("elevenLabsSpeed", 1.0, 0.7, 1.2)
            .with_ui(FieldUi::new("settings.voiceTts.field.speechRate.label").control("range")),
        boolean("elevenLabsSpeakerBoost", true).with_ui(
            FieldUi::new("settings.voiceTts.field.elevenLabsSpeakerBoost.label")
                .helper("settings.voiceTts.field.elevenLabsSpeakerBoost.helperText"),
        ),
        // `z.number()` sem `.int()` — Float, não Int, mesmo com default inteiro (3000/30000): o
        // Zod original aceitaria fracionário aqui.
        float_range("elevenLabsTextLimit", 3000.0, 500.0, 5000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.textLimit.label").control("range")),
        float_range("elevenLabsTimeoutMs", 30000.0, 5000.0, 60000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.timeoutMs.label").control("range")),
    ]
}

/// `voice-cloud-tts-config.schema.ts` (Azure)
fn azure_fields() -> Vec<FieldSchema> {
    vec![
        text("azureTtsSubscriptionKey", "")
            .with_ui(FieldUi::new("settings.voiceTts.field.azureTtsSubscriptionKey.label").control("password")),
        text("azureTtsRegion", "").with_ui(FieldUi::new("settings.voiceTts.field.azureTtsRegion.label")),
        text("azureTtsVoice", "pt-BR-FranciscaNeural").with_ui(
            FieldUi::new("settings.voiceTts.field.azureTtsVoice.label").control("select").options(AZURE_VOICES),
        ),
        float_range("azureTtsRate", 0.0, -50.0, 100.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.speechRate.label").control("range")),
        float_range("azureTtsPitch", 0.0, -50.0, 50.0)
            .with_ui(FieldUi::new("settings.voiceTts.panel.pitchLabel").control("range")),
        float_range("azureTtsTextLimit", 3000.0, 500.0, 5000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.textLimit.label").control("range")),
        float_range("azureTtsTimeoutMs", 25000.0, 5000.0, 60000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.timeoutMs.label").control("range")),
    ]
}

/// `voice-cloud-tts-config.schema.ts` (Google)
fn google_fields() -> Vec<FieldSchema> {
    vec![
        text("googleTtsApiKey", "")
            .with_ui(FieldUi::new("settings.voiceTts.field.googleTtsApiKey.label").control("password")),
        // O nome de voz real da API do Google Cloud TTS SEMPRE carrega o prefixo do idioma
        // (`pt-BR-Wavenet-A`); `voice_voices.rs` guarda os nomes completos.
        text("googleTtsVoice", "pt-BR-Wavenet-A").with_ui(
            FieldUi::new("settings.voiceTts.field.googleTtsVoice.label").control("select").options(GOOGLE_VOICES),
        ),
        text("googleTtsLanguageCode", "pt-BR").with_ui(FieldUi::new("settings.voiceTts.field.googleTtsLanguageCode.label")),
        float_range("googleTtsSpeakingRate", 1.0, 0.25, 4.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.speechRate.label").control("range")),
        float_range("googleTtsPitch", 0.0, -20.0, 20.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.googleTtsPitch.label").control("range")),
        float_range("googleTtsTextLimit", 3000.0, 500.0, 5000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.textLimit.label").control("range")),
        float_range("googleTtsTimeoutMs", 25000.0, 5000.0, 60000.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.timeoutMs.label").control("range")),
    ]
}

/// `voice-cloud-tts-config.schema.ts` (OpenAI-compatível)
fn openai_fields() -> Vec<FieldSchema> {
    vec![
        text("openaiTtsApiKey", "")
            .with_ui(FieldUi::new("settings.voiceTts.field.openaiTtsApiKey.label").control("password")),
        text("openaiTtsBaseUrl", "http://localhost:8880/v1")
            .with_ui(FieldUi::new("settings.voiceTts.field.openaiTtsBaseUrl.label").control("text")),
        text("openaiTtsVoice", "af_heart").with_ui(
            FieldUi::new("settings.voiceTts.field.openaiTtsVoice.label")
                .control("text")
                .helper("settings.voiceTts.field.openaiTtsVoice.helperText"),
        ),
        text("openaiTtsModel", "kokoro").with_ui(
            FieldUi::new("settings.voiceTts.field.openaiTtsModel.label")
                .control("text")
                .helper("settings.voiceTts.field.openaiTtsModel.helperText"),
        ),
        float_range("openaiTtsSpeed", 1.0, 0.25, 4.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.speechRate.label").control("range")),
    ]
}

/// `voice-local-tts-config.schema.ts` (Piper)
fn piper_fields() -> Vec<FieldSchema> {
    vec![
        text("piperVoice", "pt_BR-faber-medium").with_ui(
            FieldUi::new("settings.voiceTts.field.piperVoice.label")
                .control("select")
                .helper("settings.voiceTts.field.piperVoice.helperText")
                .options(&[
                    UiOption {
                        label: "settings.voiceTts.field.piperVoice.option.faber",
                        value: "pt_BR-faber-medium",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.piperVoice.option.edresson",
                        value: "pt_BR-edresson-low",
                    },
                    // Só vozes de `piper/catalog.rs` (SHA fixo + licença do dataset conferida).
                    UiOption { label: "Thorsten (de-DE)", value: "de_DE-thorsten-medium" },
                    UiOption { label: "Paola (it-IT)", value: "it_IT-paola-medium" },
                    UiOption { label: "Denis (ru-RU)", value: "ru_RU-denis-medium" },
                ]),
        ),
        text("piperBinaryPath", "").with_ui(
            FieldUi::new("settings.voiceTts.field.piperBinaryPath.label")
                .helper("settings.voiceTts.field.piperBinaryPath.helperText"),
        ),
        float_range("piperRate", 1.0, 0.5, 2.0)
            .with_ui(FieldUi::new("settings.voiceTts.field.speechRate.label").control("range")),
    ]
}

/// `voice-local-tts-config.schema.ts` (Supertonic)
fn supertonic_fields() -> Vec<FieldSchema> {
    vec![
        text("supertonicVoice", "F3").with_ui(
            FieldUi::new("settings.voiceTts.field.supertonicVoice.label")
                .control("select")
                .options(&[
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.f1",
                        value: "F1",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.f2",
                        value: "F2",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.f3",
                        value: "F3",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.f4",
                        value: "F4",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.f5",
                        value: "F5",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.m1",
                        value: "M1",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.m2",
                        value: "M2",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.m3",
                        value: "M3",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.m4",
                        value: "M4",
                    },
                    UiOption {
                        label: "settings.voiceTts.field.supertonicVoice.option.m5",
                        value: "M5",
                    },
                ]),
        ),
        float_range("supertonicRate", 1.0, 0.5, 2.0).with_ui(
            FieldUi::new("settings.voiceTts.field.supertonicRate.label")
                .control("number")
                .helper("settings.voiceTts.field.supertonicRate.helperText"),
        ),
        int_range("supertonicSteps", 20, 2, 32).with_ui(
            FieldUi::new("settings.voiceTts.field.supertonicSteps.label")
                .control("number")
                .helper("settings.voiceTts.field.supertonicSteps.helperText"),
        ),
        // Mesma lista de `supertonic::SUPPORTED_LANGUAGES`; rótulo é o nome do idioma na própria língua.
        choice("supertonicLanguage", "pt", &["pt", "en", "es", "fr", "ko"]).with_ui(
            FieldUi::new("settings.voiceTts.field.supertonicLanguage.label").control("select").options(&[
                UiOption { label: "Português", value: "pt" },
                UiOption { label: "English", value: "en" },
                UiOption { label: "Español", value: "es" },
                UiOption { label: "Français", value: "fr" },
                UiOption { label: "한국어", value: "ko" },
            ]),
        ),
    ]
}

/// `voice-stt-config.schema.ts`
fn stt_fields() -> Vec<FieldSchema> {
    vec![
        boolean("sttEnabled", true)
            .with_ui(FieldUi::new("settings.voiceStt.field.sttEnabled").described("settings.voiceStt.field.sttEnabled")),
        float_range("voiceActivityThreshold", 0.5, 0.0, 1.0).with_ui(
            FieldUi::new("settings.voiceStt.field.voiceActivityThreshold.label")
                .control("range")
                .described("settings.voiceStt.field.voiceActivityThreshold.description"),
        ),
        choice("vadEngine", "energy", &["energy", "silero"]).with_ui(
            FieldUi::new("settings.voiceStt.field.vadEngine.label")
                .control("select")
                .helper("settings.voiceStt.field.vadEngine.helperText")
                .options(&[
                    UiOption { label: "settings.voiceStt.field.vadEngine.option.energy", value: "energy" },
                    UiOption { label: "settings.voiceStt.field.vadEngine.option.silero", value: "silero" },
                ]),
        ),
        // Rejeição de digitação/clique no VAD (`modules/voice/speech_gate.rs`): barge-in, atividade do
        // usuário e o descarte de trecho só-teclado no STT. Desligado = Silero cru, janela a janela.
        boolean("vadIgnoreTyping", true).with_ui(
            FieldUi::new("settings.voiceStt.field.vadIgnoreTyping")
                .described("settings.voiceStt.calibration.ignoreTypingHint"),
        ),
        choice("inputMode", "continuous", &["ptt", "continuous"]).with_ui(
            FieldUi::new("settings.voiceStt.field.inputMode.label")
                .control("select")
                .helper("settings.voiceStt.field.inputMode.helperText")
                .options(&[
                    UiOption {
                        label: "settings.voiceStt.field.inputMode.option.continuous",
                        value: "continuous",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.inputMode.option.ptt",
                        value: "ptt",
                    },
                ]),
        ),
        // AMBÍGUO: o enum Zod aceita 5 variantes (inclui `local-whisper`), mas as `options` do
        // `m({...})` só listam 4 — o próprio TypeScript não oferece `local-whisper` na UI, embora o
        // valor seja válido. Portado como está: `choice` valida as 5, `options` mostra as 4.
        choice(
            "asrProvider",
            "nemotron",
            &["nemotron", "browser", "electron", "groq-whisper", "openai-whisper", "local-whisper"],
        )
        .with_ui(
            FieldUi::new("settings.voiceStt.field.asrProvider.label")
                .control("select")
                .options(&[
                    UiOption {
                        label: "settings.voiceStt.field.asrProvider.option.nemotron",
                        value: "nemotron",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.asrProvider.option.electron",
                        value: "electron",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.asrProvider.option.browser",
                        value: "browser",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.asrProvider.option.groq",
                        value: "groq-whisper",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.asrProvider.option.openai",
                        value: "openai-whisper",
                    },
                ]),
        ),
        // Vazio = fala no idioma de `app.locale` (`transcribe::configured_language`).
        text("asrLanguage", "").with_ui(FieldUi::new("settings.voiceStt.field.asrLanguage.label")),
        choice(
            "whisperModel",
            "onnx-community/whisper-base",
            &[
                "onnx-community/whisper-tiny",
                "onnx-community/whisper-base",
                "onnx-community/whisper-small",
            ],
        )
        .with_ui(
            FieldUi::new("settings.voiceStt.field.whisperModel.label")
                .control("select")
                .helper("settings.voiceStt.field.whisperModel.helperText")
                .options(&[
                    UiOption {
                        label: "settings.voiceStt.field.whisperModel.option.tiny",
                        value: "onnx-community/whisper-tiny",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.whisperModel.option.base",
                        value: "onnx-community/whisper-base",
                    },
                    UiOption {
                        label: "settings.voiceStt.field.whisperModel.option.small",
                        value: "onnx-community/whisper-small",
                    },
                ]),
        ),
        text("groqApiKey", "").with_ui(FieldUi::new("settings.voiceStt.field.groqApiKey.label").control("password")),
        text("openaiApiKey", "")
            .with_ui(FieldUi::new("settings.voiceStt.field.openaiApiKey.label").control("password")),
        // Catálogo único com `stt_cloud::CloudSttProvider::models` (ver `cloud_stt.rs`).
        choice("groqSttModel", cloud_stt::GROQ_MODELS[0], cloud_stt::GROQ_MODELS).with_ui(
            FieldUi::new("settings.voiceStt.field.cloudModel.label").control("select").options(cloud_stt::GROQ_OPTIONS),
        ),
        choice("openaiSttModel", cloud_stt::OPENAI_MODELS[0], cloud_stt::OPENAI_MODELS).with_ui(
            FieldUi::new("settings.voiceStt.field.cloudModel.label").control("select").options(cloud_stt::OPENAI_OPTIONS),
        ),
    ]
}

pub fn namespaces() -> Vec<Namespace> {
    let mut fields = runtime_fields();
    fields.extend(elevenlabs_fields());
    fields.extend(azure_fields());
    fields.extend(google_fields());
    fields.extend(openai_fields());
    fields.extend(piper_fields());
    fields.extend(supertonic_fields());
    fields.extend(stt_fields());

    vec![Namespace { name: "voice", fields }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn namespace() -> Namespace {
        namespaces().into_iter().next().expect("namespace voice ausente")
    }

    #[test]
    fn ports_every_field_from_the_nine_source_files() {
        let ns = namespace();
        assert_eq!(ns.name, "voice");
        // 63 - 8: `emotionDrivenTts`, `ttsTranslateAudio`, `ttsTranslationProvider`,
        // `ttsTranslationTargetLang`, `ttsDeeplxEndpoint` e `vadSpeechRms` sem leitor Rust nenhum
        // (dead config), mais `kokoroVoice`/`kokoroRate` do provedor cortado — a UI que os gravava
        // saiu junto (`renderer/settings/voice/**`); `ttsProvider` continua aceitando `"kokoro"`
        // como valor salvo (ver `runtime_fields`), só sem opção nem grupo de campos na UI.
        // + 6 parâmetros de provedor que eram fixos no código (velocidade/tom de TTS, modelo de STT),
        // + `vadEngine` (energia vs. Silero), que entrou sem subir este número nem entrar em `STT_GROUP`.
        // + `vadIgnoreTyping` (rejeição de digitação no VAD, `voice/speech_gate.rs`).
        assert_eq!(ns.fields.len(), 64);
    }

    /// A credencial do ElevenLabs é de `apiKeys.elevenLabs` (o `m({ configPath })` do Zod aponta
    /// para lá), e `apiKeys` é declarado em `llm.rs`. Declará-la aqui também faz a UI gravar num
    /// caminho que leitor nenhum consulta e some a chave que o usuário já tem.
    ///
    /// Isto é um teste por NOME, e não só o total acima, porque o total volta a bater sozinho se
    /// alguém readicionar a chave e remover outro campo — que é exatamente o erro a pegar.
    #[test]
    fn credential_does_not_live_in_the_voice_namespace() {
        let ns = namespace();

        assert!(
            ns.fields.iter().all(|field| field.key != "elevenLabsApiKey"),
            "elevenLabsApiKey voltou para `voice`; o caminho de armazenamento é apiKeys.elevenLabs"
        );
        // Piso: o resto do grupo do ElevenLabs continua aqui — a asserção acima não pode passar
        // porque o grupo inteiro sumiu.
        assert!(ns.fields.iter().any(|field| field.key == "elevenLabsVoiceId"));
    }

    #[test]
    fn runtime_defaults_match_the_typescript_schema() {
        let ns = namespace();
        let field = |key: &str| ns.fields.iter().find(|f| f.key == key).expect(key);

        assert_eq!(field("ttsProvider").default, json!("supertonic"));
        assert_eq!(field("ttsEnabled").default, json!(true));
        assert_eq!(field("volume").default, json!(1.0));
        assert_eq!(field("vadSilenceMs").default, json!(1200));
    }

    #[test]
    fn provider_defaults_match_the_typescript_schema() {
        let ns = namespace();
        let field = |key: &str| ns.fields.iter().find(|f| f.key == key).expect(key);

        assert_eq!(field("elevenLabsVoiceId").default, json!("EXAVITQu4vr4xnSDxMaL"));
        assert_eq!(field("azureTtsVoice").default, json!("pt-BR-FranciscaNeural"));
        assert_eq!(field("googleTtsVoice").default, json!("pt-BR-Wavenet-A"));
        assert_eq!(field("openaiTtsModel").default, json!("kokoro"));
        assert_eq!(field("piperVoice").default, json!("pt_BR-faber-medium"));
        assert_eq!(field("supertonicVoice").default, json!("F3"));
    }

    #[test]
    fn stt_defaults_match_the_typescript_schema() {
        let ns = namespace();
        let field = |key: &str| ns.fields.iter().find(|f| f.key == key).expect(key);

        assert_eq!(field("sttEnabled").default, json!(true));
        assert_eq!(field("inputMode").default, json!("continuous"));
        assert_eq!(field("asrProvider").default, json!("nemotron"));
        assert_eq!(field("whisperModel").default, json!("onnx-community/whisper-base"));
        assert_eq!(field("vadIgnoreTyping").default, json!(true), "rejeição de digitação nasce ligada");
    }

    /// Regressão: as opções de `googleTtsVoice` já gravaram nomes sem o prefixo (`"Wavenet-A"`), que a
    /// API do Google Cloud TTS rejeita (`GoogleTtsOptions::voice` manda o valor cru para `voice.name`).
    /// Vale para as duas nuvens: todo valor é `<idioma>-<REGIÃO>-...` e cada um dos 10 idiomas da
    /// interface tem voz.
    #[test]
    fn cloud_voice_options_carry_the_real_prefixed_names_for_all_ten_locales() {
        let options = |key: &str| namespace().fields.into_iter().find(|f| f.key == key).and_then(|f| f.ui).expect(key).options;
        let azure = options("azureTtsVoice");
        let google = options("googleTtsVoice");
        let tagged = |value: &str| {
            let mut parts = value.splitn(3, '-');
            matches!((parts.next(), parts.next(), parts.next()), (Some(l), Some(r), Some(_)) if (2..=3).contains(&l.len()) && r.len() == 2)
        };
        assert!(azure.iter().all(|o| tagged(o.value) && o.value.ends_with("Neural")), "azure fora do formato <idioma>-<REGIÃO>-<Nome>Neural");
        assert!(google.iter().all(|o| tagged(o.value) && o.value.contains("-Wavenet-")), "google fora do formato <idioma>-<REGIÃO>-Wavenet-<X>");
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            let google_locale = if locale == "zh-CN" { "cmn-CN" } else { locale };
            assert!(azure.iter().any(|o| o.value.starts_with(&format!("{locale}-"))), "azure sem voz para {locale}");
            assert!(google.iter().any(|o| o.value.starts_with(&format!("{google_locale}-"))), "google sem voz para {locale}");
        }
    }

    /// Paridade TS↔Rust: a lista que o usuário vê (`tts-provider-constants.ts`) e a que o schema
    /// declara têm de ser as mesmas vozes, nos dois sentidos — lista duplicada diverge em silêncio.
    #[test]
    fn voice_options_match_the_typescript_constants_in_both_directions() {
        let source = include_str!("../../../../src/renderer/settings/voice/tts-provider-constants.ts");
        let ts_values: std::collections::BTreeSet<&str> = source
            .split('\'')
            .filter(|token| token.ends_with("Neural") || token.contains("-Wavenet-") || token.contains("-medium") || token.contains("-low"))
            .collect();
        let ns = namespace();
        let rust_values: std::collections::BTreeSet<&str> = ["azureTtsVoice", "googleTtsVoice", "piperVoice"]
            .iter()
            .flat_map(|key| ns.fields.iter().find(|f| f.key == *key).and_then(|f| f.ui.as_ref()).expect(key).options.iter().map(|o| o.value))
            .collect();
        assert_eq!(rust_values, ts_values);
    }

    /// `local-whisper` é um valor válido no Zod (`asrProvider`), mesmo sem opção na UI — o
    /// `choice` tem de aceitá-lo mesmo que `options` não o liste.
    #[test]
    fn asr_provider_accepts_the_ui_less_variant() {
        let registry_field = namespace()
            .fields
            .into_iter()
            .find(|f| f.key == "asrProvider")
            .expect("asrProvider ausente");

        assert!(registry_field.validate("voice.asrProvider", &json!("local-whisper")).is_ok());
    }
}
