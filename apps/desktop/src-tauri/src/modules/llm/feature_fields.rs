//! Os campos de `LLM_SETTINGS` (`llm.schema.ts`), calculados à mão a partir de
//! `llm-api-keys-config.schema.ts`/`llm-model-config.schema.ts`/`llm-behavior-config.schema.ts` —
//! ver o cabeçalho de `feature.rs` para por que é literal e não uma reimplementação de
//! `zod-inference.ts`. Separado de `feature.rs` só para caber no teto de 300 linhas por arquivo.

use crate::config::schemas::llm_defaults;
use serde_json::{json, Value};

fn visible_when_provider(equals: &str) -> Value {
    json!({ "configPath": "llm.provider", "equals": equals })
}

/// As 8 credenciais/endereços (`llmApiKeyFields`), prefixo `apiKeys.` — placeholders/links vêm de
/// `LLM_PROVIDER_CATALOG` (`packages/sdk/src/llm/llm-catalog-data.ts`), copiados aqui porque são
/// literais de dado, não chave i18n (o `t()` passa reto por eles, ver `localize.rs`).
pub fn api_key_fields() -> Vec<Value> {
    vec![
        json!({ "configPath": "apiKeys.googleAI", "type": "password", "label": "settings.llm.field.googleAI.label", "sensitive": true, "placeholder": "AIzaSy...", "helperText": "settings.llm.field.googleAI.helperText", "pattern": "^AIza[0-9A-Za-z_-]{20,}$", "patternError": "settings.llm.field.googleAI.patternError", "learnMoreUrl": "https://aistudio.google.com/app/apikey", "visibleWhen": visible_when_provider("google") }),
        json!({ "configPath": "apiKeys.openrouter", "type": "password", "label": "settings.llm.field.openrouter.label", "sensitive": true, "placeholder": "sk-or-v1-...", "helperText": "settings.llm.field.openrouter.helperText", "pattern": "^sk-or-v1-[A-Za-z0-9_-]{20,}$", "patternError": "settings.llm.field.openrouter.patternError", "learnMoreUrl": "https://openrouter.ai/keys", "visibleWhen": visible_when_provider("openrouter") }),
        json!({ "configPath": "apiKeys.ollamaBaseUrl", "type": "text", "label": "settings.llm.field.ollamaBaseUrl.label", "placeholder": llm_defaults::OLLAMA_BASE_URL, "helperText": "settings.llm.field.ollamaBaseUrl.helperText", "pattern": "^https?://", "patternError": "settings.llm.field.baseUrl.patternError", "visibleWhen": visible_when_provider("ollama") }),
        json!({ "configPath": "apiKeys.ollamaApiKey", "type": "password", "label": "settings.llm.field.ollamaApiKey.label", "sensitive": true, "placeholder": "deixe vazio para Ollama local", "helperText": "settings.llm.field.ollamaApiKey.helperText", "learnMoreUrl": "https://ollama.com", "visibleWhen": visible_when_provider("ollama") }),
        json!({ "configPath": "apiKeys.openaiCompatibleBaseUrl", "type": "text", "label": "settings.llm.field.openaiCompatibleBaseUrl.label", "placeholder": "http://localhost:4000/v1", "helperText": "settings.llm.field.openaiCompatibleBaseUrl.helperText", "pattern": "^https?://", "patternError": "settings.llm.field.baseUrl.patternError", "visibleWhen": visible_when_provider("openaiCompatible") }),
        json!({ "configPath": "apiKeys.openaiCompatibleApiKey", "type": "password", "label": "Authorization Key", "sensitive": true, "placeholder": "sk-...", "helperText": "settings.llm.field.openaiCompatibleApiKey.helperText", "learnMoreUrl": "https://docs.litellm.ai/docs/proxy/quick_start", "visibleWhen": visible_when_provider("openaiCompatible") }),
        json!({ "configPath": "apiKeys.unslothBaseUrl", "type": "text", "label": "settings.llm.field.unslothBaseUrl.label", "placeholder": "http://localhost:8888/v1", "helperText": "settings.llm.field.unslothBaseUrl.helperText", "pattern": "^https?://", "patternError": "settings.llm.field.baseUrl.patternError", "visibleWhen": visible_when_provider("unsloth") }),
        json!({ "configPath": "apiKeys.unslothApiKey", "type": "password", "label": "settings.llm.field.unslothApiKey.label", "sensitive": true, "placeholder": "sk-unsloth-...", "helperText": "settings.llm.field.unslothApiKey.helperText", "learnMoreUrl": "https://unsloth.ai/docs/basics/api", "visibleWhen": visible_when_provider("unsloth") }),
    ]
}

/// O seletor de provedor da aba (`connection` group). `options` vem de `LLM_PROVIDER_OPTIONS` —
/// literais, não chave i18n (mesmo motivo do módulo).
pub fn provider_field() -> Value {
    json!({
        "configPath": "llm.provider",
        "type": "select",
        "label": "settings.llm.field.provider.label",
        "helperText": "settings.llm.field.provider.helperText",
        "options": [
            { "label": "Google AI (Gemini)", "value": "google" },
            { "label": "OpenRouter", "value": "openrouter" },
            { "label": "Ollama (local)", "value": "ollama" },
            { "label": "OpenAI Compatible", "value": "openaiCompatible" },
            { "label": "Unsloth (local)", "value": "unsloth" },
        ],
    })
}

/// Campos do grupo `model`, na ordem da tela. Só entra campo que o turno lê de verdade.
pub fn model_and_behavior_fields() -> Vec<Value> {
    vec![
        json!({ "configPath": "llm.googleModel", "type": "text", "label": "settings.llm.field.googleModel.label", "helperText": "Modelos Gemini via AI SDK com foco em multimodal e baixa latencia.", "placeholder": "gemini-3.8-flash, gemini-4-argon...", "visibleWhen": visible_when_provider("google") }),
        json!({ "configPath": "llm.openrouterModel", "type": "text", "label": "settings.llm.field.openrouterModel.label", "helperText": "Catalogo multi-provider via OpenRouter, seguindo o fluxo do vtuber-ai.", "placeholder": "google/gemini-3.8-flash, openai/gpt-5.4-mini, qwen/qwen3.8-27b...", "visibleWhen": visible_when_provider("openrouter") }),
        // `configPath` sobrescrito para `llm.model` (`ConfigKey.LlmModel`) — só este campo, ver o
        // cabeçalho de `feature.rs`.
        json!({ "configPath": "llm.model", "type": "text", "label": "settings.llm.field.ollamaModel.label", "helperText": "Modelos locais servidos por Ollama para uso offline ou privado.", "placeholder": "qwen3.5:4b, qwen3.5:9b, gemma4:12b...", "visibleWhen": visible_when_provider("ollama") }),
        json!({ "configPath": "llm.openaiCompatibleModel", "type": "text", "label": "settings.llm.field.openaiCompatibleModel.label", "helperText": "Qualquer endpoint que fale o protocolo da OpenAI: LiteLLM, vLLM, Ollama (/v1), a própria OpenAI, NVIDIA NIM.", "placeholder": "gpt-5.4-mini, gpt-5.4-nano, qualquer modelo do seu proxy...", "visibleWhen": visible_when_provider("openaiCompatible") }),
        json!({ "configPath": "llm.unslothModel", "type": "text", "label": "settings.llm.field.unslothModel.label", "helperText": "settings.llm.field.unslothModel.helperText", "placeholder": "gemma-4-26B-A4B-it-GGUF...", "visibleWhen": visible_when_provider("unsloth") }),
        json!({ "configPath": "llm.temperature", "type": "number", "label": "settings.llm.field.temperature.label", "min": 0, "max": 2, "step": 0.05, "description": "settings.llm.field.temperature.description" }),
        json!({ "configPath": "llm.topP", "type": "number", "label": "settings.llm.field.topP.label", "min": 0, "max": 1, "step": 0.01, "description": "settings.llm.field.topP.description" }),
        json!({ "configPath": "llm.maxTokens", "type": "number", "label": "settings.llm.field.maxTokens.label", "min": 256, "max": 32768, "description": "settings.llm.field.maxTokens.description" }),
        json!({ "configPath": "llm.historyLimit", "type": "number", "label": "settings.llm.field.historyLimit.label", "min": 1, "max": 100, "description": "settings.llm.field.historyLimit.description" }),
        json!({ "configPath": "llm.maxToolSteps", "type": "number", "label": "settings.llm.field.maxToolSteps.label", "min": 1, "max": 50, "description": "settings.llm.field.maxToolSteps.description" }),
        json!({ "configPath": "llm.contextWindow", "type": "number", "label": "settings.llm.field.contextWindow.label", "min": 2048, "max": 1048576, "step": 1024, "description": "settings.llm.field.contextWindow.description", "visibleWhen": visible_when_provider("ollama") }),
        json!({ "configPath": "llm.matchUserLanguage", "type": "boolean", "label": "settings.llm.field.matchUserLanguage.label", "description": "settings.llm.field.matchUserLanguage.description" }),
        json!({ "configPath": "llm.enableMonologue", "type": "boolean", "label": "settings.llm.field.enableMonologue.label", "description": "settings.llm.field.enableMonologue.description" }),
        json!({ "configPath": "llm.enableSummarization", "type": "boolean", "label": "settings.llm.field.enableSummarization.label", "description": "settings.llm.field.enableSummarization.description" }),
        json!({ "configPath": "llm.enableThinking", "type": "boolean", "label": "settings.llm.field.enableThinking.label", "description": "settings.llm.field.enableThinking.description" }),
        json!({ "configPath": "llm.thinkingMaxTokens", "type": "number", "label": "settings.llm.field.thinkingMaxTokens.label", "min": 256, "max": 100000, "description": "settings.llm.field.thinkingMaxTokens.description" }),
        json!({ "configPath": "llm.systemPrompt", "type": "textarea", "label": "settings.llm.field.systemPrompt.label", "rows": 4, "helperText": "settings.llm.field.systemPrompt.helperText", "placeholder": "Você é Mika, uma assistente VTuber AI..." }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_counts_match_the_ts_schema() {
        assert_eq!(api_key_fields().len(), 8);
        assert_eq!(model_and_behavior_fields().len(), 17);
    }

    /// O único campo com `configPath` sobrescrito — se isso regredir para `llm.ollamaModel`, o
    /// valor salvo pelo usuário em `llm.model` para de aparecer no formulário.
    #[test]
    fn ollama_model_keeps_the_overridden_config_path() {
        let fields = model_and_behavior_fields();
        let ollama_model = &fields[2];
        assert_eq!(ollama_model["configPath"], json!("llm.model"));
        assert_eq!(
            ollama_model["label"],
            json!("settings.llm.field.ollamaModel.label")
        );
    }

    #[test]
    fn provider_options_are_literal_labels_not_i18n_keys() {
        let field = provider_field();
        let options = field["options"].as_array().unwrap();
        assert_eq!(options.len(), 5);
        assert_eq!(options[4]["value"], json!("unsloth"));
        assert_eq!(options[2]["value"], json!("ollama"));
        assert_eq!(options[2]["label"], json!("Ollama (local)"));
    }
}
