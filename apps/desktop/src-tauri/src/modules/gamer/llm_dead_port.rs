//! Fixture de teste: aponta o LLM para uma porta que ninguém escuta.
//!
//! **O defeito que isto conserta.** Os testes de `describe`/`ask`/`recommend_media` que alcançam a
//! geração assertam `errorCode == "internal"`, e abriam o `ConfigStore` com o schema de PRODUÇÃO —
//! onde `llm.ollamaBaseUrl` tem default `http://localhost:11434`. Numa máquina de desenvolvimento
//! com Ollama no ar (que é o caso deste projeto), esses testes chamariam o modelo local de verdade:
//! lentos, dependentes de qual modelo está carregado, e VERMELHOS por pegarem a resposta que a
//! asserção não espera. Verde ou vermelho decidido por um processo fora do teste é flaky por
//! construção, e `AGENTS.md` trata flaky como quebrado.
//!
//! **Por que porta morta e não um dublê HTTP.** O que estes testes provam é o caminho de ERRO — que
//! a tool degrada nomeado em vez de inventar texto. Subir um servidor loopback para responder algo
//! testaria outra coisa. Sem listener, o `connect` falha na hora, igual em qualquer máquina, com ou
//! sem Ollama instalado.
//!
//! Continua sendo loopback: um endereço externo aqui passaria pela guarda de SSRF
//! (`privacy::assert_user_endpoint_allowed`) e poderia ser barrado por ela, não pela ausência de servidor — provando a
//! guarda em vez do que o teste quer provar.
//!
//! **Isto não é convenção nova: é a que já existia e a frente de gamer não seguiu.**
//! `modules/brain/turn.rs` já gravava `apiKeys.ollamaBaseUrl = http://127.0.0.1:11500` nos próprios
//! testes, pelo mesmo motivo. Este arquivo só dá um nome ao padrão para o próximo módulo achá-lo.
//!
//! Note o NAMESPACE: o campo mora em `apiKeys`, não em `llm` — `llm.ollamaBaseUrl` não existe em
//! schema nenhum e `ConfigStore::set` recusa o caminho. Foi exatamente o erro cometido ao escrever
//! este helper, e é o motivo de o teste abaixo existir.

use crate::config::store::ConfigStore;

/// Mesmo endereço de `turn.rs` — porta sem listener, `connect` devolve `ECONNREFUSED` na hora.
const DEAD_PORT_URL: &str = "http://127.0.0.1:11500";

/// Caminho REAL do campo. Ver o cabeçalho: não é `llm.`.
const OLLAMA_BASE_URL_PATH: &str = "apiKeys.ollamaBaseUrl";

pub(super) fn point_the_llm_at_a_dead_port(store: &ConfigStore) {
    store
        .set(OLLAMA_BASE_URL_PATH, serde_json::json!(DEAD_PORT_URL))
        .expect("apiKeys.ollamaBaseUrl é campo de texto do schema de produção");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem isto, o helper poderia estar gravando num caminho que o schema não declara — o
    /// `ConfigStore` recusaria, `expect` explodiria, e a mensagem apontaria para o fixture em vez do
    /// defeito. Prova que o caminho existe E que o valor gravado é o que sai na leitura.
    #[test]
    fn the_fixture_actually_overrides_the_production_default() {
        let dir = std::env::temp_dir().join(format!("mika-deadport-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();

        let default_url = store.get(OLLAMA_BASE_URL_PATH);
        point_the_llm_at_a_dead_port(&store);

        assert_eq!(store.get(OLLAMA_BASE_URL_PATH), Some(serde_json::json!(DEAD_PORT_URL)));
        assert_ne!(default_url, Some(serde_json::json!(DEAD_PORT_URL)), "o default já era a porta morta — o fixture virou no-op e o teste não prova mais nada");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
