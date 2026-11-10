//! Redação de segredo para o que entra no ring buffer de `log_buffer.rs` (painel de Logs de
//! Configurações) — API key, token de bearer, cabeçalho `Authorization` e par `key=valor` de
//! query string. Log de shell não é PII de janela (isso é `system_signals::redact`); é log de
//! `log::log!` do processo inteiro, que pode conter erro de rede com a credencial embutida.
//!
//! **Cópia intencional, não importação.** Mesmo padrão de duplicação núcleo-núcleo aceito em
//! `system_signals::redact` (que documenta o porquê): os padrões `sk-`, `gh*_` e JWT vieram de
//! `memory::social::fact_store::secret_patterns`; os de chave de provedor (`sk-or-v1-`, `AIza`, `gsk_`)
//! foram acrescentados aqui, porque o log recebe a mensagem de erro CRUA do provedor.

use std::sync::OnceLock;

use regex::Regex;

fn fixed_patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // `sk-`/`sk_` + corpo com `-`/`_` no meio: OpenAI (`sk-proj-…`), Anthropic, OpenRouter
            // (`sk-or-v1-…`) e ElevenLabs (`sk_…`). Google (`AIza…`) e Groq (`gsk_…`) têm prefixo próprio.
            (Regex::new(r"\bsk[-_][A-Za-z0-9_-]{20,}").unwrap(), "[KEY]"),
            (Regex::new(r"\bAIza[A-Za-z0-9_-]{30,}").unwrap(), "[KEY]"),
            (Regex::new(r"\bgsk_[A-Za-z0-9]{20,}").unwrap(), "[KEY]"),
            (Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b").unwrap(), "[KEY]"),
            (Regex::new(r"\bey[A-Za-z0-9_-]{10,}\.ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap(), "[JWT]"),
            // Cabeçalho `Authorization` completo, com ou sem o esquema `Bearer` — resolvido ANTES
            // do padrão genérico de chave/valor abaixo para não sobrar um token pela metade.
            (Regex::new(r"(?i)\bauthorization\s*[=:]\s*(?:bearer\s+)?\S+").unwrap(), "authorization: [KEY]"),
            // Token `Bearer ...` fora de um cabeçalho `Authorization` (ex.: dentro de uma URL ou
            // corpo de erro logado cru).
            (Regex::new(r"(?i)\bbearer\s+\S+").unwrap(), "Bearer [KEY]"),
        ]
    })
}

// `key=valor`/`token=valor`/`secret=valor`/`password=valor` (query param ou log key-value)."authorization"
// fica de fora: o padrão fixo acima já resolveu esse campo, incluir os dois duplicaria o marcador.
fn key_value_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([a-z0-9_-]*(?:token|key|secret|password)\s*[=:]\s*)[^\s&,;]+").unwrap())
}

/// Aplica os padrões acima, nessa ordem, a uma linha de log antes dela tocar o ring buffer.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for (pattern, replacement) in fixed_patterns() {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    key_value_pattern().replace_all(&out, "$1[KEY]").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_an_anthropic_style_key() {
        let key = format!("sk-ant-{}", "a".repeat(40));
        assert_eq!(redact(&format!("usando a chave {key}")), "usando a chave [KEY]");
    }

    #[test]
    fn redacts_a_github_token() {
        let token = format!("ghp_{}", "a".repeat(36));
        assert_eq!(redact(&format!("token {token} inválido")), "token [KEY] inválido");
    }

    #[test]
    fn redacts_a_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        assert_eq!(redact(&format!("id token recebido: {jwt}")), "id token recebido: [JWT]");
    }

    #[test]
    fn redacts_an_authorization_header_with_bearer_scheme() {
        assert_eq!(
            redact("falha ao chamar api: Authorization: Bearer abc123.def456"),
            "falha ao chamar api: authorization: [KEY]"
        );
    }

    #[test]
    fn redacts_an_authorization_header_without_bearer_scheme() {
        assert_eq!(redact("header authorization=abc123xyz enviado"), "header authorization: [KEY] enviado");
    }

    #[test]
    fn redacts_a_bare_bearer_token_outside_a_header() {
        assert_eq!(redact("retry com bearer abc.def.ghi falhou"), "retry com Bearer [KEY] falhou");
    }

    #[test]
    fn redacts_a_key_value_query_param() {
        assert_eq!(redact("GET /v1/models?apiKey=abcdef1234567890"), "GET /v1/models?apiKey=[KEY]");
    }

    #[test]
    fn redacts_a_token_field() {
        assert_eq!(redact("refresh token=abcdef1234567890 salvo"), "refresh token=[KEY] salvo");
    }

    /// Chave solta numa mensagem de erro do provedor ("Incorrect API key provided: sk-or-v1-…") não
    /// tem `key=` na frente, então só o padrão de prefixo a pega. O de antes exigia 32+ alfanuméricos
    /// logo depois de `sk-`: OpenRouter (`sk-or-v1-`), OpenAI de projeto (`sk-proj-…_…`) e ElevenLabs
    /// (`sk_`) passavam em claro para `mika-ai.ndjson`; Google (`AIza`) e Groq (`gsk_`) nem tinham padrão.
    #[test]
    fn redacts_a_bare_key_of_every_provider_the_app_talks_to() {
        let keys = [
            format!("sk-or-v1-{}", "a1b2c3d4".repeat(8)),
            format!("sk-proj-{}_{}-x", "Ab1".repeat(10), "Cd2".repeat(10)),
            format!("sk-ant-api03-{}", "Zz9".repeat(20)),
            format!("sk_{}", "0a1b2c".repeat(8)),
            format!("AIza{}", "Sy-Ab_c1".repeat(5).chars().take(35).collect::<String>()),
            format!("gsk_{}", "Q1w2E3".repeat(8)),
        ];
        for key in keys {
            let masked = redact(&format!("provedor recusou a chave {key} (401)"));
            assert_eq!(masked, "provedor recusou a chave [KEY] (401)", "vazou: {key}");
        }
    }

    #[test]
    fn leaves_plain_text_without_secrets_unchanged() {
        assert_eq!(redact("modelo carregado em 120ms"), "modelo carregado em 120ms");
    }
}
