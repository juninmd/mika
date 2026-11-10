//! `topP`/`enableThinking`/`thinkingMaxTokens` no dialeto de cada provedor remoto.
//!
//! Antes, só `openaiCompatible` recebia algo (`chat_template_kwargs`): os três controles da aba LLM
//! não tinham efeito nenhum com Google/OpenRouter. Cada provedor fala um formato próprio, e mandar
//! o campo errado é 400, não parâmetro ignorado — então raciocínio só sai quando o PERFIL do modelo
//! ([`ModelProfile`], perguntado ao provedor) confirma suporte. Perfil desconhecido = não envia.
//!
//! - OpenRouter: `provider: {zdr, data_collection:"deny"}` sempre (política de dados, ver
//!   [`openrouter`]), `top_p` e `reasoning` (`{max_tokens}` ligado) — formato unificado de
//!   `openrouter.ai/docs/use-cases/reasoning-tokens`. Desligado só manda `{enabled:false}` onde o
//!   upstream deixa desligar (Gemini 2.5 Flash): modelo de raciocínio obrigatório (2.5 Pro, o3,
//!   gpt-5) responde 400 "Reasoning is mandatory" — nesses, desligado = default do modelo.
//! - Google: `generationConfig.topP` e `generationConfig.thinkingConfig.thinkingBudget`. Desligar
//!   (budget 0) só existe na família `gemini-2.5-flash`: 2.5 Pro exige budget mínimo e Gemini 3 não
//!   desliga (`ai.google.dev/gemini-api/docs/gemini-3`) — nesses, desligado = default do modelo.
//!   O orçamento é cortado no teto da família (Flash 24576, demais 32768): o slider vai a 100000.
//! - openaiCompatible (llama.cpp/vLLM): `top_p` e `chat_template_kwargs.enable_thinking=false`; o
//!   template não tem orçamento de raciocínio, então `thinkingMaxTokens` não se aplica.

use serde_json::{json, Map, Value};

use crate::config::store::ConfigStore;
use crate::modules::llm::model_profile::ModelProfile;

#[derive(Debug, Clone, PartialEq)]
pub struct SamplingSettings {
    pub top_p: Option<f64>,
    pub enable_thinking: bool,
    pub thinking_max_tokens: u64,
}

impl SamplingSettings {
    pub fn from_store(store: &ConfigStore) -> Self {
        Self {
            top_p: store.get("llm.topP").and_then(|v| v.as_f64()),
            enable_thinking: store.get("llm.enableThinking").and_then(|v| v.as_bool()).unwrap_or(false),
            thinking_max_tokens: store.get("llm.thinkingMaxTokens").and_then(|v| v.as_f64()).map_or(4096, |n| n as u64),
        }
    }
}

fn google_can_disable_thinking(model: &str) -> bool {
    model.trim_start_matches("google/").starts_with("gemini-2.5-flash")
}

/// Maior orçamento de raciocínio que o modelo aceita: teto da família Gemini e saída máxima do perfil.
pub fn thinking_budget_cap(model: &str, profile: &ModelProfile) -> u64 {
    let gemini = model.trim_start_matches("google/");
    let family_cap = if gemini.starts_with("gemini-2.5-flash") { 24_576 } else if gemini.starts_with("gemini-") { 32_768 } else { u64::MAX };
    family_cap.min(profile.max_output_tokens.unwrap_or(u64::MAX))
}

fn thinking_budget(model: &str, settings: &SamplingSettings, profile: &ModelProfile) -> u64 {
    settings.thinking_max_tokens.min(thinking_budget_cap(model, profile))
}

/// Política de dados carimbada em TODO pedido ao OpenRouter (chat e visão): só endpoints com
/// retenção zero (`zdr`) e nenhum que colete dado para treino. É o par do filtro `?zdr=true` do
/// catálogo (`modules/llm.rs`): sem este campo no corpo, o filtro só esconde modelos e o pedido
/// continua livre para cair num endpoint que retém o conteúdo. A política só mora aqui (a constante
/// TS `OPENROUTER_PRIVACY` era código morto e saiu); o teste de `brain::turn` prova o corpo no fio.
fn openrouter(model: &str, settings: &SamplingSettings, profile: &ModelProfile, params: &mut Map<String, Value>) {
    params.insert("provider".into(), json!({ "zdr": true, "data_collection": "deny" }));
    if let Some(top_p) = settings.top_p.filter(|_| profile.top_p != Some(false)) {
        params.insert("top_p".into(), json!(top_p));
    }
    if profile.reasoning != Some(true) {
        return;
    }
    if settings.enable_thinking {
        params.insert("reasoning".into(), json!({ "max_tokens": thinking_budget(model, settings, profile) }));
    } else if google_can_disable_thinking(model) {
        params.insert("reasoning".into(), json!({ "enabled": false }));
    }
}

fn google(model: &str, settings: &SamplingSettings, profile: &ModelProfile, params: &mut Map<String, Value>) {
    let mut config = Map::new();
    if let Some(top_p) = settings.top_p {
        config.insert("topP".into(), json!(top_p));
    }
    if profile.reasoning == Some(true) {
        if settings.enable_thinking {
            config.insert("thinkingConfig".into(), json!({ "thinkingBudget": thinking_budget(model, settings, profile) }));
        } else if google_can_disable_thinking(model) {
            config.insert("thinkingConfig".into(), json!({ "thinkingBudget": 0 }));
        }
    }
    if !config.is_empty() {
        params.insert("generationConfig".into(), Value::Object(config));
    }
}

fn openai_compatible(settings: &SamplingSettings, params: &mut Map<String, Value>) {
    if let Some(top_p) = settings.top_p {
        params.insert("top_p".into(), json!(top_p));
    }
    if !settings.enable_thinking {
        params.insert("chat_template_kwargs".into(), json!({ "enable_thinking": false }));
    }
}

/// Parâmetros de amostragem/raciocínio de um provedor remoto. Ollama tem caminho próprio
/// (`turn::build_ollama_additional_params`) porque `num_ctx`/`think` são do dialeto dele.
pub fn remote_params(provider: &str, model: &str, settings: &SamplingSettings, profile: &ModelProfile) -> Map<String, Value> {
    let mut params = Map::new();
    match provider {
        "openrouter" => openrouter(model, settings, profile, &mut params),
        "google" => google(model, settings, profile, &mut params),
        "openaiCompatible" => openai_compatible(settings, &mut params),
        // Só `top_p`, que é campo padrão da API OpenAI. `chat_template_kwargs` é dialeto do
        // llama-server cru: o Unsloth fica na frente dele e a doc não diz que repassa campo
        // desconhecido, então não mando o que não consegui verificar (corpo inválido derruba o turno).
        "unsloth" => {
            if let Some(top_p) = settings.top_p {
                params.insert("top_p".into(), json!(top_p));
            }
        }
        _ => {}
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zdr() -> Value {
        json!({ "zdr": true, "data_collection": "deny" })
    }

    fn settings(enable_thinking: bool) -> SamplingSettings {
        SamplingSettings { top_p: Some(0.9), enable_thinking, thinking_max_tokens: 2048 }
    }

    fn reasoning_model() -> ModelProfile {
        ModelProfile { reasoning: Some(true), top_p: Some(true), ..Default::default() }
    }

    #[test]
    fn unsloth_gets_only_the_standard_top_p_never_the_llama_server_dialect() {
        let params = remote_params("unsloth", "gemma", &settings(false), &ModelProfile::default());
        assert_eq!(params.get("top_p"), Some(&json!(0.9)));
        assert!(!params.contains_key("chat_template_kwargs"));
        assert!(!params.contains_key("id_slot"));
    }

    #[test]
    fn openrouter_turns_thinking_off_on_a_reasoning_model_and_budgets_it_when_on() {
        let off = remote_params("openrouter", "google/gemini-2.5-flash", &settings(false), &reasoning_model());
        assert_eq!(Value::Object(off), json!({ "provider": zdr(), "top_p": 0.9, "reasoning": { "enabled": false } }));
        let on = remote_params("openrouter", "google/gemini-2.5-flash", &settings(true), &reasoning_model());
        assert_eq!(on["reasoning"], json!({ "max_tokens": 2048 }));
    }

    /// Raciocínio obrigatório (o3, gpt-5, 2.5 Pro) recusa `{enabled:false}` com 400 — desligado não manda nada.
    #[test]
    fn openrouter_leaves_mandatory_reasoning_models_on_their_default_when_thinking_is_off() {
        for model in ["openai/o3", "openai/gpt-5", "google/gemini-2.5-pro"] {
            let off = remote_params("openrouter", model, &settings(false), &reasoning_model());
            assert_eq!(Value::Object(off), json!({ "provider": zdr(), "top_p": 0.9 }), "{model}");
        }
    }

    /// Slider vai a 100000; Gemini recusa orçamento acima do teto da família e o perfil limita a saída.
    #[test]
    fn thinking_budget_is_capped_at_the_family_limit_and_the_model_output() {
        let big = SamplingSettings { thinking_max_tokens: 100_000, ..settings(true) };
        let flash = remote_params("google", "gemini-2.5-flash", &big, &reasoning_model());
        assert_eq!(flash["generationConfig"]["thinkingConfig"]["thinkingBudget"], json!(24_576));
        let pro = remote_params("google", "gemini-2.5-pro", &big, &reasoning_model());
        assert_eq!(pro["generationConfig"]["thinkingConfig"]["thinkingBudget"], json!(32_768));
        let small_output = ModelProfile { max_output_tokens: Some(8_000), ..reasoning_model() };
        let routed = remote_params("openrouter", "anthropic/claude-sonnet-4", &big, &small_output);
        assert_eq!(routed["reasoning"], json!({ "max_tokens": 8_000 }));
    }

    /// `reasoning` para modelo sem raciocínio é 400 no OpenRouter — nem perfil desconhecido arrisca.
    #[test]
    fn openrouter_never_sends_reasoning_or_top_p_the_model_does_not_accept() {
        let plain = ModelProfile { reasoning: Some(false), top_p: Some(false), ..Default::default() };
        let gemma = remote_params("openrouter", "google/gemma-3-27b-it", &settings(false), &plain);
        assert_eq!(Value::Object(gemma), json!({ "provider": zdr() }));
        let unknown = remote_params("openrouter", "x/y", &settings(false), &ModelProfile::default());
        assert_eq!(Value::Object(unknown), json!({ "provider": zdr(), "top_p": 0.9 }));
    }

    #[test]
    fn google_sends_top_p_and_thinking_budget_inside_generation_config() {
        let on = remote_params("google", "gemini-2.5-pro", &settings(true), &reasoning_model());
        assert_eq!(Value::Object(on), json!({ "generationConfig": { "topP": 0.9, "thinkingConfig": { "thinkingBudget": 2048 } } }));
        let off = remote_params("google", "gemini-2.5-flash", &settings(false), &reasoning_model());
        assert_eq!(off["generationConfig"]["thinkingConfig"], json!({ "thinkingBudget": 0 }));
    }

    /// 2.5 Pro recusa budget 0 e Gemini 3 não desliga raciocínio: desligado = default do modelo.
    #[test]
    fn google_never_sends_a_zero_budget_to_models_that_cannot_turn_thinking_off() {
        for model in ["gemini-2.5-pro", "gemini-3-pro-preview"] {
            let off = remote_params("google", model, &settings(false), &reasoning_model());
            assert_eq!(Value::Object(off), json!({ "generationConfig": { "topP": 0.9 } }), "{model}");
        }
    }

    #[test]
    fn openai_compatible_keeps_the_template_switch_and_gains_top_p() {
        let off = remote_params("openaiCompatible", "gemma", &settings(false), &ModelProfile::default());
        assert_eq!(Value::Object(off), json!({ "top_p": 0.9, "chat_template_kwargs": { "enable_thinking": false } }));
        let on = remote_params("openaiCompatible", "gemma", &settings(true), &ModelProfile::default());
        assert_eq!(Value::Object(on), json!({ "top_p": 0.9 }));
    }
}
