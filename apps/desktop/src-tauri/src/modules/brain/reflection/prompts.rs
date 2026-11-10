//! Monta o PEDIDO de reflexão: o recorte da conversa e a amarração com a cadência. O carregamento
//! dos `.md` em si mora em [`crate::modules::brain::prompt_loader`], compartilhado com
//! [`super::super::post_turn::facts`] (ambos os prompts vivem no mesmo namespace `brain`).

use std::collections::HashMap;

use tauri::AppHandle;

use super::cadence::ReflectionCadence;
use super::super::prompt_loader::load_prompt;

const CONVERSATION_SNIPPET_CHARS: usize = 400;
const PROMPT_SYSTEM_KEY: &str = "reflection-system";
const PROMPT_USER_KEY: &str = "reflection-subconscious";

/// Porte de `` `Usuário: ${user.slice(0, 400)}\nMika: ${assistant.slice(0, 400)}` ``. Corta por
/// CARACTERE, não por byte: `String.prototype.slice` do TS corta por unidade UTF-16, e cortar por
/// byte em Rust partiria um caractere multi-byte (acento, emoji) ao meio — a mesma classe de
/// cuidado que `memory.rs` documenta para preservar unicode em `set`/`hSet`.
pub fn build_conversation_text(user: &str, assistant: &str) -> String {
    let user_snippet: String = user.chars().take(CONVERSATION_SNIPPET_CHARS).collect();
    let assistant_snippet: String = assistant.chars().take(CONVERSATION_SNIPPET_CHARS).collect();
    format!("User: {user_snippet}\nMika: {assistant_snippet}")
}

/// Os dois prompts que `maybeRun` monta antes de chamar `generateObject`/`turn::run_turn_llm_raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionRequest {
    pub system_prompt: String,
    pub user_prompt: String,
}

/// Resultado de tentar montar o pedido de reflexão para este turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectionStep {
    /// Não é a vez do ciclo — mesmo `return null` cedo de `maybeRun`.
    NotDue,
    /// Um dos dois prompts (`.md`) não existe ou está em branco — mesma degradação de
    /// `if (!prompt) return null`, mas relatada em vez de engolida.
    PromptUnavailable(String),
    /// Pronto para ir ao modelo.
    Ready(ReflectionRequest),
}

/// Monta o pedido de reflexão deste turno: avança a cadência e, se for a vez, carrega os dois
/// prompts (`reflection-subconscious` com `conversationText`, `reflection-system` sem variáveis —
/// mesma ordem de `maybeRun`, que monta o prompt do usuário antes de resolver o modelo).
pub fn prepare_reflection_step(
    app: &AppHandle,
    cadence: &mut ReflectionCadence,
    user: &str,
    assistant: &str,
) -> ReflectionStep {
    if !cadence.tick() {
        return ReflectionStep::NotDue;
    }

    let mut vars = HashMap::new();
    vars.insert("conversationText".to_string(), build_conversation_text(user, assistant));

    let user_prompt = match load_prompt(app, PROMPT_USER_KEY, &vars) {
        Ok(prompt) => prompt,
        Err(error) => return ReflectionStep::PromptUnavailable(error),
    };
    let system_prompt = match load_prompt(app, PROMPT_SYSTEM_KEY, &HashMap::new()) {
        Ok(prompt) => prompt,
        Err(error) => return ReflectionStep::PromptUnavailable(error),
    };

    ReflectionStep::Ready(ReflectionRequest { system_prompt, user_prompt })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_conversation_text_monta_o_template_com_os_dois_lados() {
        let text = build_conversation_text("oi", "olá!");
        assert_eq!(text, "User: oi\nMika: olá!");
    }

    #[test]
    fn build_conversation_text_corta_cada_lado_em_400_caracteres() {
        let long_user = "a".repeat(500);
        let long_assistant = "b".repeat(500);
        let text = build_conversation_text(&long_user, &long_assistant);

        let user_part = text.strip_prefix("User: ").unwrap().split("\nMika: ").next().unwrap();
        let assistant_part = text.split("\nMika: ").nth(1).unwrap();
        assert_eq!(user_part.chars().count(), CONVERSATION_SNIPPET_CHARS);
        assert_eq!(assistant_part.chars().count(), CONVERSATION_SNIPPET_CHARS);
    }

    /// Corte por CARACTERE, não por byte — um emoji (4 bytes, 1 char) na borda do limite não pode
    /// virar bytes inválidos de UTF-8.
    #[test]
    fn build_conversation_text_nao_parte_caractere_multibyte_na_borda_do_corte() {
        let mut user = "x".repeat(CONVERSATION_SNIPPET_CHARS - 1);
        user.push('🎮');
        user.push_str("resto que deve sumir");

        let text = build_conversation_text(&user, "");
        let user_part = text.strip_prefix("User: ").unwrap().split("\nMika: ").next().unwrap();
        assert!(user_part.ends_with('🎮'));
        assert_eq!(user_part.chars().count(), CONVERSATION_SNIPPET_CHARS);
    }

    // `prepare_reflection_step` precisa de `AppHandle` (via `load_prompt`), que não dá para forjar
    // num teste de unidade sem subir o Tauri — mesma limitação que `modules/memory.rs::memory_store`
    // documenta para `AppHandle`. O gate de cadência já é coberto sem `AppHandle` por `cadence.rs`;
    // o carregamento de prompt em si (`load_prompt_in`) é coberto sem `AppHandle` em
    // `crate::modules::brain::prompt_loader`.
}
