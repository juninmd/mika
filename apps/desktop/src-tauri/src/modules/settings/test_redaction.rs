//! Higiene de segredo na mensagem que o teste de conexão devolve para a TELA.
//!
//! Sai de `plugin_test.rs` por responsabilidade, não só pelo teto de 300 linhas: aquele arquivo
//! decide se a conexão funciona, este decide o que pode ser dito sobre a falha. Misturar os dois
//! faz a redação virar detalhe de um ramo de teste, quando ela vale para todos.
//!
//! **Duas camadas, porque a primeira sozinha depende da FORMA do segredo.** Trocar a chave exata só
//! acerta quando ela aparece literal: `?api_key=abc123` é pego, `?api_key=abc%31%32%33` não, e o
//! mesmo vale para qualquer encoding que o cliente HTTP aplique a caminho do `Display` do erro.
//! Redação por igualdade de string é, na prática, uma allowlist do formato que o autor imaginou.
//!
//! A segunda camada não depende disso: **toda query string some**, com chave configurada ou sem. É
//! o único lugar onde uma credencial cabe numa URL de erro, nada que o usuário precise para
//! diagnosticar conexão mora ali (host, porta e caminho ficam), e vale para segredo que este
//! módulo nem sabe que existe. Esta árvore já vazou chave do Google por `Error.message`; o conserto
//! que dura não é lembrar de mascarar cada chave nova.


use crate::config::store::ConfigStore;

/// A chave que o ramo `llm` pode fazer o cliente embutir numa URL. Só esta: o ramo só fala Ollama.
const OLLAMA_API_KEY_PATH: &str = "apiKeys.ollamaApiKey";

/// Limpa a mensagem de erro antes de ela virar texto de tela.
pub(super) fn mask_secret(message: String, store: &ConfigStore) -> String {
    let key = store.get(OLLAMA_API_KEY_PATH).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let message = if key.is_empty() { message } else { message.replace(&key, "[REDACTED]") };
    strip_url_queries(&message)
}

/// Corta `?...` de toda URL do texto, preservando o resto da mensagem.
///
/// O fim da URL é o primeiro caractere que não pode estar nela — espaço, aspas, parêntese, fim de
/// linha. O corte só vale DEPOIS de `http://`/`https://`: sem isso, um `?` de pergunta em prosa
/// (`"o servidor está rodando? tente de novo"`) engoliria o resto da frase, e essa frase é o único
/// diagnóstico que o usuário tem nessa tela.
fn strip_url_queries(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;

    while let Some(start) = ["http://", "https://"].into_iter().filter_map(|scheme| rest.find(scheme)).min() {
        let (before, from_url) = rest.split_at(start);
        out.push_str(before);

        let url_end = from_url.find([' ', '\t', '\n', '"', '\'', ')', '>']).unwrap_or(from_url.len());
        let (url, tail) = from_url.split_at(url_end);
        match url.find('?') {
            Some(query) => {
                out.push_str(&url[..query]);
                out.push_str("?[REDACTED]");
            }
            None => out.push_str(url),
        }
        rest = tail;
    }

    out.push_str(rest);
    out
}

/// Só para o teste de `plugin_test.rs` montar um store — a redação em si não grava nada.
#[cfg(test)]
pub(super) fn set_ollama_key(store: &ConfigStore, key: &str) {
    store.set(OLLAMA_API_KEY_PATH, serde_json::Value::String(key.to_string())).expect("gravar a chave de teste");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schemas::registry;
    use crate::config::store::ConfigStore;

    fn store() -> (ConfigStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mika-test-redaction-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).expect("diretório temporário");
        (ConfigStore::open(&dir, registry()).expect("ConfigStore"), dir)
    }

    #[test]
    fn the_configured_key_never_reaches_the_screen_verbatim() {
        let (store, _dir) = store();
        set_ollama_key(&store, "sk-super-secret");

        let masked = mask_secret("erro ao conectar em http://host/sk-super-secret/v1".into(), &store);
        assert!(!masked.contains("sk-super-secret"), "chave vazou: {masked}");
        assert!(masked.contains("[REDACTED]"));
    }

    /// **A camada que não depende da forma do segredo.** Percent-encoded, a troca por igualdade
    /// erra; a query some do mesmo jeito.
    #[test]
    fn a_percent_encoded_key_still_never_reaches_the_screen() {
        let (store, _dir) = store();
        set_ollama_key(&store, "abc123");

        let masked = mask_secret("falhou: http://host:11434/api/chat?api_key=abc%31%32%33".into(), &store);
        assert!(!masked.contains("abc%31%32%33"), "chave codificada vazou: {masked}");
        assert!(masked.contains("http://host:11434/api/chat"), "host e caminho sumiram junto: {masked}");
    }

    /// Query some mesmo sem NENHUMA chave configurada: vale para segredo que este módulo não sabe
    /// que existe.
    #[test]
    fn any_url_query_is_dropped_even_with_no_key_configured() {
        let (store, _dir) = store();

        let masked = mask_secret("GET https://api.exemplo.com/v1?token=xyz falhou".into(), &store);
        assert!(!masked.contains("xyz"), "token vazou: {masked}");
        assert!(masked.contains("https://api.exemplo.com/v1"));
        assert!(masked.ends_with(" falhou"), "o texto depois da URL foi engolido: {masked}");
    }

    /// NEGATIVO — `?` de prosa não é query.
    #[test]
    fn a_question_mark_in_prose_is_not_treated_as_a_query_string() {
        let (store, _dir) = store();
        let message = "o servidor está rodando? tente de novo".to_string();
        assert_eq!(mask_secret(message.clone(), &store), message);
    }

    /// Mensagem sem chave configurada e sem URL passa intacta — redação não pode comer diagnóstico.
    #[test]
    fn a_plain_message_passes_through_untouched() {
        let (store, _dir) = store();
        let message = "erro qualquer".to_string();
        assert_eq!(mask_secret(message.clone(), &store), message);
    }

    /// Duas URLs na mesma mensagem: as duas perdem a query, e o texto entre elas sobrevive.
    #[test]
    fn every_url_in_the_message_loses_its_query_not_just_the_first() {
        let (store, _dir) = store();

        let masked = mask_secret("de http://a/x?k=1 para https://b/y?k=2 falhou".into(), &store);
        assert!(!masked.contains("k=1") && !masked.contains("k=2"), "sobrou query: {masked}");
        assert!(masked.contains("http://a/x") && masked.contains("https://b/y"));
        assert!(masked.contains(" para ") && masked.ends_with(" falhou"));
    }

    /// `https` ANTES de `http`: a busca pegava o `http://` mais adiante e a query da URL https, que
    /// vinha primeiro, passava inteira para a tela.
    #[test]
    fn an_https_url_before_an_http_one_loses_its_query_too() {
        let (store, _dir) = store();

        let masked = mask_secret("de https://b/y?k=2 para http://a/x?k=1 falhou".into(), &store);
        assert!(!masked.contains("k=1") && !masked.contains("k=2"), "sobrou query: {masked}");
        assert!(masked.contains("https://b/y") && masked.contains("http://a/x"));
    }
}
