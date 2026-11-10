//! Porte de `apps/desktop/src/main/services/voice/tts/tts-state.ts` — só os campos que algum
//! caminho Rust de fato lê hoje (Supertonic, Piper, os 4 provedores remotos e os campos de
//! pré-processamento de texto). Os campos do Kokoro (AINDA cortado — ver `dispatch.rs`) não foram
//! portados: um campo de config sem leitor é o mesmo defeito que
//! `project_homologacao_config_ui_criterio` já catalogou do outro lado (schema sem UI), aqui
//! invertido (schema sem leitor Rust).

use crate::config::store::ConfigStore;
use crate::modules::voice::tts::preprocessor::TextPreprocessorOptions;
use crate::modules::voice::tts::providers::remote::{AzureTtsOptions, ElevenLabsOptions, GoogleTtsOptions, OpenAiTtsOptions};

pub const DEFAULT_SUPERTONIC_STEPS: u32 = 20;

#[derive(Clone)]
pub struct TtsState {
    pub provider: String,
    pub supertonic_voice: String,
    pub supertonic_rate: f32,
    pub supertonic_steps: u32,
    pub supertonic_language: String,
    pub supertonic_text_limit: usize,
    pub piper_voice: String,
    pub piper_binary_path: String,
    pub piper_rate: f32,
    pub eleven_labs: ElevenLabsOptions,
    pub openai_tts: OpenAiTtsOptions,
    pub azure_tts: AzureTtsOptions,
    pub google_tts: GoogleTtsOptions,
    pub preprocessor: TextPreprocessorOptions,
}

impl Default for TtsState {
    fn default() -> Self {
        Self {
            provider: "supertonic".into(),
            supertonic_voice: "F3".into(),
            supertonic_rate: 1.0,
            supertonic_steps: DEFAULT_SUPERTONIC_STEPS,
            supertonic_language: "pt".into(),
            supertonic_text_limit: 3000,
            piper_voice: "pt_BR-faber-medium".into(),
            piper_binary_path: String::new(),
            piper_rate: 1.0,
            eleven_labs: ElevenLabsOptions::default(),
            openai_tts: OpenAiTtsOptions::default(),
            azure_tts: AzureTtsOptions::default(),
            google_tts: GoogleTtsOptions::default(),
            preprocessor: TextPreprocessorOptions::default(),
        }
    }
}

fn get_str(store: &ConfigStore, path: &str, fallback: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| fallback.to_string())
}

fn get_f32(store: &ConfigStore, path: &str, fallback: f32) -> f32 {
    store.get(path).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(fallback)
}

fn get_u32(store: &ConfigStore, path: &str, fallback: u32) -> u32 {
    store.get(path).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(fallback)
}

fn get_bool(store: &ConfigStore, path: &str, fallback: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(fallback)
}

/// Lê `voice.*` do `ConfigStore` do processo — mesmos caminhos de `config/schemas/voice.rs`
/// (`runtime_fields`/`supertonic_fields`). Defaults locais só entram quando a chave falta (config
/// corrompido/parcial); o schema já garante o valor de produção normal.
impl TtsState {
    pub fn from_config(store: &ConfigStore) -> Self {
        let defaults = Self::default();
        Self {
            provider: get_str(store, "voice.ttsProvider", &defaults.provider),
            supertonic_voice: get_str(store, "voice.supertonicVoice", &defaults.supertonic_voice),
            supertonic_rate: get_f32(store, "voice.supertonicRate", defaults.supertonic_rate),
            supertonic_steps: get_u32(store, "voice.supertonicSteps", defaults.supertonic_steps),
            supertonic_language: crate::config::locale::follow_locale(
                get_str(store, "voice.supertonicLanguage", &defaults.supertonic_language),
                &defaults.supertonic_language,
                &crate::config::locale::app_locale(store),
                |locale| crate::config::locale::language_code(locale).to_string(),
            ),
            supertonic_text_limit: defaults.supertonic_text_limit,
            piper_voice: get_str(store, "voice.piperVoice", &defaults.piper_voice),
            piper_binary_path: get_str(store, "voice.piperBinaryPath", &defaults.piper_binary_path),
            piper_rate: get_f32(store, "voice.piperRate", defaults.piper_rate),
            eleven_labs: ElevenLabsOptions::from_config(store),
            openai_tts: OpenAiTtsOptions::from_config(store),
            azure_tts: AzureTtsOptions::from_config(store),
            google_tts: GoogleTtsOptions::from_config(store),
            preprocessor: TextPreprocessorOptions {
                ignore_brackets: get_bool(store, "voice.ttsIgnoreBrackets", defaults.preprocessor.ignore_brackets),
                ignore_parentheses: get_bool(store, "voice.ttsIgnoreParentheses", defaults.preprocessor.ignore_parentheses),
                ignore_asterisks: get_bool(store, "voice.ttsIgnoreAsterisks", defaults.preprocessor.ignore_asterisks),
                ignore_angle_brackets: get_bool(
                    store,
                    "voice.ttsIgnoreAngleBrackets",
                    defaults.preprocessor.ignore_angle_brackets,
                ),
                remove_emoji: get_bool(store, "voice.ttsRemoveEmoji", defaults.preprocessor.remove_emoji),
            },
        }
    }

    pub fn effective_speed(&self, prosody_rate: Option<f32>) -> f32 {
        let rate = match prosody_rate {
            Some(p) if (p - 1.0).abs() > f32::EPSILON => self.supertonic_rate * p,
            _ => self.supertonic_rate,
        };
        rate.clamp(0.5, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_the_typescript_defaults() {
        let state = TtsState::default();
        assert_eq!(state.provider, "supertonic");
        assert_eq!(state.supertonic_voice, "F3");
        assert_eq!(state.supertonic_steps, 20);
        assert_eq!(state.piper_voice, "pt_BR-faber-medium");
        assert_eq!(state.piper_binary_path, "");
        assert_eq!(state.piper_rate, 1.0);
    }

    /// `voice.piper*` chega ao `TtsState` como qualquer outro provedor — mesma leitura que
    /// `config/schemas/voice.rs::piper_fields` já declara, do outro lado (ver o cabeçalho).
    #[test]
    fn from_config_reads_the_piper_fields() {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::{float_range, text};
        let dir = std::env::temp_dir().join(format!("mika-tts-state-piper-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "voice",
            fields: vec![
                text("piperVoice", "pt_BR-faber-medium"),
                text("piperBinaryPath", ""),
                float_range("piperRate", 1.0, 0.5, 2.0),
            ],
        }]);
        let store = ConfigStore::open(&dir, schema).unwrap();
        store.set("voice.piperVoice", serde_json::json!("de_DE-thorsten-medium")).unwrap();
        store.set("voice.piperBinaryPath", serde_json::json!("/opt/piper/piper")).unwrap();
        store.set("voice.piperRate", serde_json::json!(1.5)).unwrap();

        let state = TtsState::from_config(&store);
        assert_eq!(state.piper_voice, "de_DE-thorsten-medium");
        assert_eq!(state.piper_binary_path, "/opt/piper/piper");
        assert_eq!(state.piper_rate, 1.5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O idioma era sempre "pt": a Mika lia inglês com fonemas de português.
    #[test]
    fn from_config_reads_the_supertonic_language() {
        let dir = std::env::temp_dir().join(format!("mika-tts-state-lang-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert_eq!(TtsState::from_config(&store).supertonic_language, "pt");
        store.set("voice.supertonicLanguage", serde_json::json!("en")).unwrap();
        assert_eq!(TtsState::from_config(&store).supertonic_language, "en");
        assert!(store.set("voice.supertonicLanguage", serde_json::json!("de")).is_err(), "idioma fora do modelo aceito");

        // O "pt" que ninguém escolheu segue a interface; o que não é o default fica como está.
        store.set("voice.supertonicLanguage", serde_json::json!("pt")).unwrap();
        store.set("app.locale", serde_json::json!("ko-KR")).unwrap();
        assert_eq!(TtsState::from_config(&store).supertonic_language, "ko");
        store.set("voice.supertonicLanguage", serde_json::json!("fr")).unwrap();
        assert_eq!(TtsState::from_config(&store).supertonic_language, "fr");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn effective_speed_clamps_to_the_same_range_as_get_supertonic_speed() {
        let mut state = TtsState::default();
        state.supertonic_rate = 3.0;
        assert_eq!(state.effective_speed(None), 2.0);
        state.supertonic_rate = 0.1;
        assert_eq!(state.effective_speed(None), 0.5);
    }

    #[test]
    fn effective_speed_applies_a_non_default_prosody_hint() {
        let state = TtsState::default();
        assert_eq!(state.effective_speed(Some(1.2)), 1.2);
    }
}
