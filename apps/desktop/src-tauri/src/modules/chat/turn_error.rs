//! A mensagem que o usuário lê quando um turno de chat falha. Antes, `send.rs` colava
//! `err.to_string()` no chat: texto cru do provedor, em português fixo, e — no Gemini — com a URL da
//! requisição inteira, `?key=<segredo>` incluso. Aqui o erro vira uma frase localizada
//! (`settings::t`, chaves `chat.error.*`) e o que sobra de detalhe passa por `log_redact::redact`.
//!
//! A classificação lê o texto CRU só para decidir a chave; nada dele é mostrado sem redação.

use crate::modules::brain::turn::TurnError;
use crate::modules::llm::provider_health::{extract_http_status, is_data_policy_mismatch};
use crate::modules::settings::t;
use crate::modules::system::log_redact::redact;

/// Teto do detalhe técnico anexado ao erro genérico — o suficiente para diagnosticar, não para
/// despejar o corpo de uma resposta HTTP inteira no balão.
const DETAIL_MAX_CHARS: usize = 200;

const NETWORK_MARKERS: [&str; 8] = ["error sending request", "connection refused", "connect error", "dns", "timed out", "timeout", "unreachable", "connection reset"];
const KEY_MARKERS: [&str; 5] = ["api key", "api_key", "unauthorized", "authentication", "permission denied"];

/// `provider` é o provedor configurado (`ollama`, `google`, …): a mesma falha de conexão é "Ollama
/// ausente" no local e "rede" num provedor remoto.
pub(super) fn user_message(err: &TurnError, provider: &str, locale: &str) -> String {
    match err {
        TurnError::Provider(reason) | TurnError::ProviderBeforeSideEffects(reason) => provider_message(reason, provider, locale),
        other => redact(&other.to_string()),
    }
}

fn provider_message(reason: &str, provider: &str, locale: &str) -> String {
    match classify(reason, provider) {
        Some(key) => t(key, locale),
        None => {
            let detail: String = redact(reason).chars().take(DETAIL_MAX_CHARS).collect();
            format!("{} ({detail})", t("chat.error.generic", locale))
        }
    }
}

fn classify(reason: &str, provider: &str) -> Option<&'static str> {
    if is_data_policy_mismatch(reason) {
        return Some("chat.error.noZdrEndpoint");
    }
    let lower = reason.to_lowercase();
    if matches!(extract_http_status(reason), Some(401 | 403)) || KEY_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Some("chat.error.invalidKey");
    }
    if NETWORK_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Some(if provider == "ollama" { "chat.error.ollamaMissing" } else { "chat.error.network" });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(reason: &str) -> TurnError {
        TurnError::Provider(reason.to_string())
    }

    const GEMINI_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/m:streamGenerateContent?key=SEGREDO123&alt=sse";

    #[test]
    fn a_network_failure_on_a_remote_provider_is_the_network_message_without_the_url_key() {
        let message = user_message(&provider(&format!("HttpError: error sending request for url ({GEMINI_URL})")), "google", "pt-BR");
        assert_eq!(message, t("chat.error.network", "pt-BR"));
        assert!(!message.contains("SEGREDO123"), "{message}");
    }

    #[test]
    fn a_rejected_key_is_the_invalid_key_message_whatever_the_status_text_says() {
        let by_status = format!("HttpError: Invalid status code 403 Forbidden with message: denied for {GEMINI_URL}");
        let by_text = "Invalid status code 400 Bad Request with message: API key not valid. Please pass a valid API key.";
        for reason in [by_status.as_str(), by_text] {
            let message = user_message(&provider(reason), "google", "en-US");
            assert_eq!(message, t("chat.error.invalidKey", "en-US"), "{reason}");
            assert!(!message.contains("SEGREDO123"), "{message}");
        }
    }

    #[test]
    fn the_same_connection_failure_is_ollama_missing_only_for_the_local_provider() {
        let reason = "HttpError: error sending request for url (http://localhost:11434/api/chat): Connection refused";
        assert_eq!(user_message(&provider(reason), "ollama", "en-US"), t("chat.error.ollamaMissing", "en-US"));
        assert_eq!(user_message(&provider(reason), "openrouter", "en-US"), t("chat.error.network", "en-US"));
    }

    #[test]
    fn an_unclassified_failure_keeps_a_redacted_and_capped_detail() {
        let reason = format!("modelo recusou o pedido ({GEMINI_URL}) {}", "x".repeat(500));
        let message = user_message(&provider(&reason), "google", "en-US");
        assert!(message.starts_with(&t("chat.error.generic", "en-US")), "{message}");
        assert!(message.contains("modelo recusou"), "o detalhe ajuda a diagnosticar: {message}");
        assert!(!message.contains("SEGREDO123"), "{message}");
        assert!(message.chars().count() < t("chat.error.generic", "en-US").chars().count() + DETAIL_MAX_CHARS + 5, "{message}");
    }

    /// B2: OpenRouter sem endpoint ZDR (404) caía em `chat.error.generic` com o JSON cru — a pessoa
    /// não sabia que bastava trocar de modelo. Agora a frase é acionável, nos 10 locales.
    #[test]
    fn a_model_without_a_zdr_endpoint_gets_an_actionable_message_in_every_locale() {
        let reason = "HttpError: Invalid status code 404 Not Found with message: {\"error\":{\"message\":\"No endpoints found matching your data policy (Zero data retention). Configure: https://openrouter.ai/settings/privacy\",\"code\":404}}";
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            let message = user_message(&provider(reason), "openrouter", locale);
            assert_eq!(message, t("chat.error.noZdrEndpoint", locale), "{locale}");
            assert_ne!(message, "chat.error.noZdrEndpoint", "{locale}: chave sem tradução");
            assert!(!message.contains("openrouter.ai"), "{locale}: {message}");
        }
    }

    #[test]
    fn the_message_follows_the_requested_locale() {
        let reason = "error sending request for url (https://api.example.com)";
        assert_ne!(user_message(&provider(reason), "google", "pt-BR"), user_message(&provider(reason), "google", "en-US"));
    }

    #[test]
    fn errors_that_are_not_from_the_provider_keep_their_own_text() {
        assert_eq!(user_message(&TurnError::Aborted, "ollama", "en-US"), TurnError::Aborted.to_string());
    }
}
