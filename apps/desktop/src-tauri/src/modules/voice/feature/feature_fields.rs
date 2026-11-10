//! Campos e mapa de agrupamento das abas `voice`/`stt` — separado de `feature.rs` só para caber
//! no teto de 300 linhas (mesmo motivo da divisão `llm/feature.rs` + `llm/feature_fields.rs`).
//!
//! **Por que a serialização de campo é DUPLICADA aqui, e não importada de
//! `features::settings_declaration::declare_fields`.** É o modelo que a tarefa apontou
//! (`features/character/feature.rs`), mas `voice` é núcleo bloqueante (`AGENTS.md` §2.1) e
//! `settings_declaration` mora em `modules::features` — a camada de "features tardias". O
//! invariante do mesmo parágrafo é direto: "Núcleo nunca importa de feature." `llm`/`vision`
//! (também núcleo) já respeitam isso hoje — nenhum dos dois chama `declare_fields`; os dois montam
//! o `Value` à mão (ver o cabeçalho de `llm/feature.rs`). [`field_json`]/[`field_type`]/
//! [`insert_bounds`] abaixo são a MESMA lógica de `settings_declaration.rs`, copiada porque é puro
//! formato (que tipo `FieldKind` vira `SettingsFieldType`, onde `min`/`max` entram) — não uma
//! segunda fonte da verdade sobre quais campos existem: essa continua sendo só
//! `config::schemas::voice::namespaces()`, lida via `SchemaRegistry::declarations()`, a MESMA
//! chamada que `declare_fields` faz por baixo.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::config::registry::SchemaRegistry;
use crate::config::schema::{FieldKind, FieldSchema};

pub(super) const NAMESPACE: &str = "voice";

fn field_type(field: &FieldSchema) -> &'static str {
    const VALID: &[&str] =
        &["text", "number", "range", "boolean", "select", "password", "textarea", "url", "object"];
    if let Some(control) = field.ui.as_ref().and_then(|ui| ui.control) {
        if VALID.contains(&control) {
            return control;
        }
    }
    match field.kind {
        FieldKind::Bool => "boolean",
        FieldKind::Int { .. } | FieldKind::Float { .. } => "number",
        FieldKind::Str { .. } => "text",
        FieldKind::Enum { .. } => "select",
        FieldKind::List | FieldKind::Map => "object",
    }
}

fn insert_bounds(out: &mut serde_json::Map<String, Value>, field: &FieldSchema) {
    match field.kind {
        FieldKind::Int { min, max } => {
            if let Some(min) = min {
                out.insert("min".into(), json!(min));
            }
            if let Some(max) = max {
                out.insert("max".into(), json!(max));
            }
        }
        FieldKind::Float { min, max } => {
            if let Some(min) = min {
                out.insert("min".into(), json!(min));
            }
            if let Some(max) = max {
                out.insert("max".into(), json!(max));
            }
        }
        FieldKind::Str { min_len, max_len } => {
            if let Some(min_len) = min_len {
                out.insert("minLength".into(), json!(min_len));
            }
            if let Some(max_len) = max_len {
                out.insert("maxLength".into(), json!(max_len));
            }
        }
        FieldKind::Bool | FieldKind::Enum { .. } | FieldKind::List | FieldKind::Map => {}
    }
}

fn field_json(field: &FieldSchema) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("configPath".into(), json!(format!("{NAMESPACE}.{}", field.key)));
    out.insert("type".into(), json!(field_type(field)));
    insert_bounds(&mut out, field);

    match &field.ui {
        Some(ui) => {
            out.insert("label".into(), json!(ui.label));
            if let Some(description) = ui.description {
                out.insert("description".into(), json!(description));
            }
            if let Some(helper_text) = ui.helper_text {
                out.insert("helperText".into(), json!(helper_text));
            }
            if !ui.options.is_empty() {
                let options: Vec<Value> =
                    ui.options.iter().map(|option| json!({ "label": option.label, "value": option.value })).collect();
                out.insert("options".into(), json!(options));
            }
        }
        None => {
            out.insert("label".into(), json!(field.key));
        }
    }

    Value::Object(out)
}

/// Os 55 campos do namespace `voice` (`config/schemas/voice.rs`), sem agrupar — igual ao que
/// `declare_fields` devolveria, se pudesse ser chamado daqui. `None` se o namespace não existir.
pub(super) fn declare_fields(schema: &SchemaRegistry) -> Option<Vec<Value>> {
    let declaration = schema.declarations().into_iter().find(|entry| entry.namespace == NAMESPACE)?;
    Some(declaration.fields.iter().map(field_json).collect())
}

pub(super) fn key_of(field: &Value) -> String {
    field["configPath"].as_str().and_then(|path| path.rsplit('.').next()).unwrap_or_default().to_string()
}

pub(super) struct GroupSpec {
    pub(super) title: &'static str,
    /// Chaves (sem prefixo `voice.`), na MESMA ordem do `z.object({...})` de origem.
    pub(super) keys: &'static [&'static str],
    /// `{ configPath: 'voice.ttsProvider', equals: <este valor> }` — `None` para os grupos sem
    /// `visibleWhen` (`runtime` e o `asr` da aba `stt`).
    pub(super) visible_when_equals: Option<&'static str>,
    /// `testAction: { label }` — só o grupo `elevenlabs` tem, em `voice.schema.ts`.
    pub(super) test_action_label: Option<&'static str>,
}

pub(super) fn build_group(spec: &GroupSpec, by_key: &mut HashMap<String, Value>) -> Value {
    let fields: Vec<Value> = spec.keys.iter().filter_map(|key| by_key.remove(*key)).collect();
    let mut group = json!({
        "title": spec.title,
        "groupPrefix": NAMESPACE,
        "fields": fields,
    });
    if let Some(equals) = spec.visible_when_equals {
        group["visibleWhen"] = json!({ "configPath": "voice.ttsProvider", "equals": equals });
    }
    if let Some(label) = spec.test_action_label {
        group["testAction"] = json!({ "label": label });
    }
    group
}

/// `VOICE_SETTINGS.groups` (`voice.schema.ts`), na mesma ordem: runtime, elevenlabs, supertonic,
/// piper, azure, google, openai. 47 campos ao todo (17+8+3+3+5+7+4) — `kokoro` saiu do grupo
/// próprio (provedor cortado, `dispatch.rs::CUT_PROVIDERS`) sem sair do enum `ttsProvider`.
pub(super) const VOICE_GROUPS: &[GroupSpec] = &[
    GroupSpec {
        title: "settings.voiceTts.group.runtime",
        keys: &[
            "ttsProvider",
            "ttsEnabled",
            "volume",
            "autoSpeak",
            "autoMuteDuringTts",
            "interruptionEnabled",
            "interruptionThreshold",
            "interruptionTriggerStreak",
            "interruptionCooldownMs",
            "vadEndpointingEnabled",
            "vadSilenceMs",
            "vadSpeechThreshold",
            "ttsIgnoreBrackets",
            "ttsIgnoreParentheses",
            "ttsIgnoreAsterisks",
            "ttsIgnoreAngleBrackets",
            "ttsRemoveEmoji",
        ],
        visible_when_equals: None,
        test_action_label: None,
    },
    GroupSpec {
        title: "settings.voiceTts.group.elevenlabs",
        // `elevenLabsApiKey` FICA DE FORA: no TS ele mora no MESMO `z.object` (`VoiceElevenLabsGroupSchema`),
        // mas `m({ configPath: 'apiKeys.elevenLabs' })` reaponta o armazenamento para outro namespace
        // (`apiKeys`, declarado em `llm.rs`) — nunca existiu em `config/schemas/voice.rs`. Ver o
        // cabeçalho de `voice.rs::elevenlabs_fields` para os detalhes.
        keys: &[
            "elevenLabsVoiceId",
            "elevenLabsModel",
            "ttsStability",
            "elevenLabsSimilarityBoost",
            "elevenLabsStyle",
            "elevenLabsSpeed",
            "elevenLabsSpeakerBoost",
            "elevenLabsTextLimit",
            "elevenLabsTimeoutMs",
        ],
        visible_when_equals: Some("elevenlabs"),
        test_action_label: Some("settings.voiceTts.testAction.label"),
    },
    GroupSpec {
        title: "settings.voiceTts.group.supertonic",
        keys: &["supertonicVoice", "supertonicLanguage", "supertonicRate", "supertonicSteps"],
        visible_when_equals: Some("supertonic"),
        test_action_label: None,
    },
    GroupSpec {
        title: "settings.voiceTts.group.piper",
        keys: &["piperVoice", "piperBinaryPath", "piperRate"],
        visible_when_equals: Some("piper"),
        test_action_label: None,
    },
    GroupSpec {
        title: "settings.voiceTts.group.azure",
        keys: &["azureTtsSubscriptionKey", "azureTtsRegion", "azureTtsVoice", "azureTtsRate", "azureTtsPitch", "azureTtsTextLimit", "azureTtsTimeoutMs"],
        visible_when_equals: Some("azure-tts"),
        test_action_label: None,
    },
    GroupSpec {
        title: "settings.voiceTts.group.google",
        keys: &[
            "googleTtsApiKey",
            "googleTtsVoice",
            "googleTtsLanguageCode",
            "googleTtsSpeakingRate",
            "googleTtsPitch",
            "googleTtsTextLimit",
            "googleTtsTimeoutMs",
        ],
        visible_when_equals: Some("google-tts"),
        test_action_label: None,
    },
    GroupSpec {
        title: "settings.voiceTts.group.openai",
        keys: &["openaiTtsApiKey", "openaiTtsBaseUrl", "openaiTtsVoice", "openaiTtsModel", "openaiTtsSpeed"],
        visible_when_equals: Some("openai-tts"),
        test_action_label: None,
    },
];

/// `STT_SETTINGS.groups[0]` (`voice.schema.ts`) — `VoiceAsrGroupSchema` inteiro, 8 campos, sem
/// `visibleWhen`.
pub(super) const STT_GROUP: GroupSpec = GroupSpec {
    title: "settings.voiceStt.group.asr",
    keys: &[
        "sttEnabled",
        "voiceActivityThreshold",
        "vadEngine",
        "vadIgnoreTyping",
        "inputMode",
        "asrProvider",
        "asrLanguage",
        "whisperModel",
        "groqApiKey",
        "openaiApiKey",
        "groqSttModel",
        "openaiSttModel",
    ],
    visible_when_equals: None,
    test_action_label: None,
};
