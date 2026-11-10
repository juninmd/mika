//! === monologue.ts (`generateMonologue`) ==========================================================
//!
//! Gate: `llm.enableMonologue` (`ConfigKey.LlmEnableMonologue`, default `false` no schema TS e em
//! `config/schemas/llm.rs::llm()`) — o TS só chama `generateMonologue` quando
//! `llmFactory.isMonologueEnabled()` é `true`; aqui o chamador (`turn.rs::run_turn_ollama`/
//! `run_turn_llm`) lê o MESMO campo antes de chamar [`generate`]. Default desligado: sem opt-in,
//! zero chamadas extras de LLM — nenhuma latência/custo novo por padrão.
//!
//! ## Custo por turno quando ligado
//!
//! Uma chamada de LLM NÃO-streaming inteira A MAIS por turno (mesmo provedor/modelo do turno
//! principal — sem `createRoutedModel`/tabela de roteamento, que não existe em Rust ainda, mesma
//! lacuna já documentada no cabeçalho de `turn.rs`). `max_tokens=120` mantém a resposta curta, mas
//! ainda é uma requisição de rede síncrona ANTES do turno principal começar (`run_turn_ollama` chama
//! [`generate`] antes de montar o prompt) — o usuário espera a latência das DUAS chamadas somadas,
//! não só uma. Reportado ao coordenador; não há retry nem streaming aqui, mesma economia do TS.
//!
//! ## Degradação silenciosa, de propósito
//!
//! Qualquer erro (`.md` ausente, provedor fora do ar, resposta vazia) devolve `None` — o monólogo é
//! só o bloco opcional "## Internal Reasoning"; ele nunca pode derrubar o turno principal. Mesmo
//! contrato do `catch` em `generateMonologue` (loga e devolve `null`).
//!
//! ## Sem `memoryHints`
//!
//! O TS passa `memoryHintsFrom(providerTrace)` (memória social agregada) como bloco extra do prompt
//! de usuário. Rust não tem `buildSocialMemoryContext` portado — mesma lacuna já aceita em
//! `system_prompt.rs` para o perfil de usuário/gíria. Omitido, não inventado.

use rig_core::completion::{CompletionRequest, CompletionResponse, Message};
use rig_core::message::{AssistantContent, UserContent};

use crate::modules::brain::llm_provider::ChatModel;

const MAX_OUTPUT_TOKENS: u64 = 120;
const TEMPERATURE: f64 = 0.75;
const HISTORY_WINDOW: usize = 6;
const HISTORY_CHAR_LIMIT: usize = 200;

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn first_text<'a>(items: impl IntoIterator<Item = &'a UserContent>) -> Option<String> {
    items.into_iter().find_map(|c| match c {
        UserContent::Text(t) => Some(t.text.clone()),
        _ => None,
    })
}

fn first_assistant_text<'a>(items: impl IntoIterator<Item = &'a AssistantContent>) -> Option<String> {
    items.into_iter().find_map(|c| match c {
        AssistantContent::Text(t) => Some(t.text.clone()),
        _ => None,
    })
}

/// Espelha o `.map`/`.join('\n')` de `generateMonologue`: só as últimas [`HISTORY_WINDOW`] mensagens
/// de texto puro (`Message::System` e mensagens sem conteúdo de texto — só tool call/result — são
/// ignoradas, como o `ChatMessage` flat do TS nunca carregaria um tool call de qualquer forma).
fn recent_context(history: &[Message]) -> String {
    let start = history.len().saturating_sub(HISTORY_WINDOW);
    history[start..]
        .iter()
        .filter_map(|message| match message {
            Message::User { content } => first_text(content).map(|text| ("User", text)),
            Message::Assistant(reply) => first_assistant_text(&reply.content).map(|text| ("Mika", text)),
            Message::System { .. } => None,
        })
        .map(|(author, text)| format!("{author}: {}", truncate_chars(&text, HISTORY_CHAR_LIMIT)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Espelha o template de `userPrompt` em `generateMonologue` (sem o bloco de `memoryHints` — ver o
/// cabeçalho do arquivo).
fn build_user_prompt(user_content: &str, history: &[Message]) -> String {
    format!("Recent context:\n{}\n\nUser just said: \"{user_content}\"\n\nInternal reasoning:", recent_context(history))
}

fn extract_text(response: &CompletionResponse) -> Option<String> {
    response.choice.iter().find_map(|c| match c {
        AssistantContent::Text(t) => Some(t.text.clone()),
        _ => None,
    })
}

/// Núcleo testável sobre um [`ChatModel`] — mesmo padrão de `run_turn` (dublê de teste em vez de
/// rede real). `monologue.md` chega via `chat::load_prompt_no_handle` (mesma fachada sem
/// `AppHandle` do resto de `brain::persona`), FALHA silenciosa (`None`) se o `.md` não existir, igual
/// a qualquer outro erro deste caminho.
pub(super) async fn generate(model: ChatModel, user_content: &str, history: &[Message]) -> Option<String> {
    let system_prompt = crate::modules::chat::load_prompt_no_handle("brain", "monologue", &std::collections::HashMap::new()).ok()?;
    let prompt = build_user_prompt(user_content, history);

    let request = CompletionRequest::new(Message::user(prompt)).preamble(system_prompt).max_tokens(MAX_OUTPUT_TOKENS).temperature(TEMPERATURE);

    let response = model.call(request).await.ok()?;
    let text = extract_text(&response)?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::test_utils::{MockCompletionModel, MockTurn};

    fn user(text: &str) -> Message {
        Message::User { content: vec![UserContent::Text(rig_core::message::Text::new(text))] }
    }

    fn assistant(text: &str) -> Message {
        Message::from(vec![AssistantContent::Text(rig_core::message::Text::new(text))])
    }

    #[test]
    fn recent_context_keeps_only_the_last_six_messages_labeled_by_author() {
        let history: Vec<Message> = (0..8).map(|i| if i % 2 == 0 { user(&format!("u{i}")) } else { assistant(&format!("a{i}")) }).collect();
        let ctx = recent_context(&history);

        assert!(!ctx.contains("u0"), "as duas primeiras mensagens caem fora da janela de 6");
        assert!(ctx.contains("User: u2"));
        assert!(ctx.contains("Mika: a7"));
        assert_eq!(ctx.lines().count(), 6);
    }

    #[test]
    fn recent_context_truncates_long_messages_to_two_hundred_chars() {
        let long = "x".repeat(500);
        let ctx = recent_context(&[user(&long)]);
        assert_eq!(ctx, format!("User: {}", "x".repeat(HISTORY_CHAR_LIMIT)));
    }

    #[test]
    fn build_user_prompt_matches_the_typescript_template_shape() {
        let prompt = build_user_prompt("oi mika", &[user("olá")]);
        assert!(prompt.starts_with("Recent context:\nUser: olá"));
        assert!(prompt.ends_with("User just said: \"oi mika\"\n\nInternal reasoning:"));
    }

    // -- generate: ponta a ponta com um modelo de mentira (`MockCompletionModel` do `rig-core`) -----

    fn fake(reply: Result<&str, ()>) -> ChatModel {
        let turn = match reply {
            Ok(text) => MockTurn::text(text),
            Err(()) => MockTurn::error("falha simulada"),
        };
        MockCompletionModel::from_turns([turn]).erase()
    }

    #[tokio::test]
    async fn generate_returns_the_trimmed_reply_via_the_real_prompt_loader() {
        let text = generate(fake(Ok("  vou provocar sobre o bug de novo  ")), "oi", &[]).await;
        assert_eq!(text.as_deref(), Some("vou provocar sobre o bug de novo"), "deveria carregar monologue.md via o fallback de dev e devolver o texto aparado");
    }

    /// Degradação silenciosa: erro do provedor vira `None`, nunca um turno quebrado (mesmo `catch`
    /// de `generateMonologue`).
    #[tokio::test]
    async fn generate_degrades_silently_to_none_on_a_provider_error() {
        assert_eq!(generate(fake(Err(())), "oi", &[]).await, None);
    }

    #[tokio::test]
    async fn generate_treats_a_blank_reply_as_none() {
        assert_eq!(generate(fake(Ok("   ")), "oi", &[]).await, None);
    }
}
