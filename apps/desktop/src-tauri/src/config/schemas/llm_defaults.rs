//! Defaults de provedor de LLM — fonte ÚNICA no backend. Antes viviam copiados em seis arquivos
//! (`brain/turn.rs`, `vision/llm_client.rs`, `gamer/game_watcher.rs`, `chat/send.rs`,
//! `brain/llm_provider.rs`, o schema) e uma troca de modelo padrão atualizava uns e esquecia outros:
//! o chat ia para um modelo e a visão/o gamer para outro, sem erro nenhum.
//!
//! Espelham o `defaultModel` de `packages/sdk/src/llm/llm-catalog-data.ts` (o que o renderer lê).

/// Ollama local na porta padrão do daemon.
pub(crate) const OLLAMA_BASE_URL: &str = "http://localhost:11434";
pub(crate) const OLLAMA_MODEL: &str = "qwen3.5:4b";
pub(crate) const GOOGLE_MODEL: &str = "gemini-3.8-flash";
pub(crate) const OPENROUTER_MODEL: &str = "google/gemini-3.8-flash";
pub(crate) const OPENAI_COMPATIBLE_MODEL: &str = "gpt-5.4-mini";
