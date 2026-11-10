//! As 5 tools de `BrainFeature` (`apps/desktop/src/main/services/brain/brain.feature.ts`),
//! registradas no `ToolRegistry` nativo (`crate::modules::tools::registry()`) com `plugin_id =
//! "mika.brain"` — o mesmo id que `bootstrap/tools.ts::registerFeatureWithTools('mika.brain',
//! BrainFeature)` grava em cada tool, não `BrainFeature.id` ('brain').
//!
//! ## Tabela de paridade TS × Rust (medida nesta rodada)
//!
//! | Tool | Ação | Depende de | Rust hoje |
//! |---|---|---|---|
//! | `match_effect` | (todas) | nada — função pura | **portado, paridade total** |
//! | `web` | `search`/`fetch` | motor de navegador (Chromium/Playwright) | gate de config real, execução degrada `unavailable` nomeado |
//! | `memory_console` | `search_episodic`/`clear_episodic`/`delete_episodic` | `modules::memory::episodic` | **portado, paridade total** |
//! | `memory_console` | `add_fact` | `modules::memory::social` (grafo social) | **portado, paridade total** |
//! | `memory_console` | `snapshot` | episódica (real) + grafo social (real) + `promptTrace` de turno (não persistido) | **parcial, nomeado** — `episodic`/`graph` reais, `notPorted: ["promptTrace"]` |
//! | `content` | `daily_recap` | `HistoryService` (histórico) + LLM | **portado, paridade real** (esta rodada — ver `content.rs`) |
//! | `content` | `social_post` | LLM | **portado, paridade real** |
//! | `collaboration` | `start`/`stop` | `CollaborationService` (LLM, debate multi-turno) | **portado, paridade real** (`EventKey::AssistantMessage` publicado por turno — ver `collaboration_session.rs`/`assistant_message.rs` — e cada turno gravado em `chat.db` via `collaboration_history.rs`) |
//!
//! De 5 tools, 4 com paridade total ou real do mecanismo (`match_effect`, `memory_console`,
//! `content`, `collaboration`) e 1 com paridade parcial real (`web`'s gate de config;
//! `memory_console`'s `snapshot` também é parcial, mas pelo pedaço que sobra ser estado de turno em
//! memória — `promptTrace` — não algo persistido que um porte Rust pudesse alcançar).
//!
//! `web`, `content` e `collaboration` precisam do `ConfigStore` (gate `browser.enabled`; persona e
//! idioma do prompt; chamada real ao LLM) — as três vão para [`register_config_bound_tools`],
//! chamado da segunda passada de boot (mesmo ponto que já liga `vision::register`). `content`
//! também precisa do `AppHandle` (resolver `chat.db` para `daily_recap`, ver `content.rs`), por
//! isso a assinatura da função ganhou `app` nesta rodada. As outras 2 (`match_effect`,
//! `memory_console`) não precisam de config nenhum: registram como efeito colateral de
//! `modules()`, mesmo padrão de `gamer::modules()`.

mod assistant_message;
mod collaboration;
mod collaboration_history;
mod collaboration_session;
mod content;
mod match_effect;
mod memory_console;
mod prompts;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::store::ConfigStore;
use crate::modules::tools::ToolDefinition;

const PLUGIN_ID: &str = "mika.brain";

fn definition(name: &str, description: &str, input_schema: serde_json::Value) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: Some(description.into()),
        input_schema,
        plugin_id: PLUGIN_ID.into(),
        requires_window: false,
    }
}

/// As 2 tools sem `ConfigStore` — chame de `modules()` (efeito colateral, sem `AppHandle`).
pub(super) fn register_config_free_tools() {
    let registry = crate::modules::tools::registry();
    registry.register(
        definition("match_effect", match_effect::DESCRIPTION, match_effect::schema()),
        Arc::new(match_effect::MatchEffectHandler),
    );
    registry.register(
        definition("memory_console", memory_console::DESCRIPTION, memory_console::schema()),
        Arc::new(memory_console::MemoryConsoleHandler),
    );
}

/// `web` (gate `browser.enabled`), `content` (persona, idioma do prompt, `chat_db_path` para
/// `daily_recap`) e `collaboration` (persona/LLM do debate) precisam do `ConfigStore` — chame da
/// segunda passada de boot, uma vez, com o `ConfigStore` do processo. `chat_db_path` já vem
/// RESOLVIDO por quem chama (`modules/mod.rs::register_config_bound_tools`, o único ponto com
/// `AppHandle`) — `None` quando a resolução falhou no boot, e só `daily_recap` degrada por isso.
pub(super) fn register_config_bound_tools(store: Arc<ConfigStore>, chat_db_path: Option<PathBuf>) {
    let registry = crate::modules::tools::registry();
    registry.register(
        definition("web", web::DESCRIPTION, web::schema()),
        Arc::new(web::WebHandler::new(Arc::clone(&store))),
    );

    registry.register(
        definition("content", content::DESCRIPTION, content::schema()),
        Arc::new(content::ContentHandler::new(Arc::clone(&store), chat_db_path.clone())),
    );

    registry.register(
        definition("collaboration", collaboration::DESCRIPTION, collaboration::schema()),
        // Mesmo `chat_db_path` de `content` (mesmo arquivo `chat.db`) — cada turno do debate grava
        // ali via `collaboration_history::persist_debate_turn`; ver o cabeçalho de
        // `collaboration_session.rs`.
        Arc::new(collaboration::CollaborationHandler::new(store, chat_db_path)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-brain-tools-register-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn config_free_registration_publishes_both_tools_under_mika_brain() {
        register_config_free_tools();
        let all = crate::modules::tools::registry().get_all();
        for name in ["match_effect", "memory_console"] {
            let tool = all.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} registrada"));
            assert_eq!(tool.plugin_id, "mika.brain");
        }

        // `content`/`collaboration` saíram desta lista de propósito: passaram a fechar sobre o
        // `ConfigStore` (persona, idioma do prompt, LLM) e só nascem na segunda passada de boot —
        // a prova está em `config_bound_registration_publishes_web_content_and_collaboration`.
        // Aqui NÃO cabe asserir que estão ausentes: `registry()` é `OnceLock` de processo e os
        // testes rodam em paralelo, então o teste vizinho pode tê-las registrado antes. Negativa
        // contra singleton é flake, não prova.
    }

    #[tokio::test]
    async fn config_bound_registration_publishes_web_content_and_collaboration() {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace { name: "browser", fields: vec![boolean("enabled", true)] }]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        store.set("browser.enabled", json!(false)).unwrap();

        // `chat_db_path` resolvido "na mão", sem `AppHandle` (o crate não habilita `test-support`
        // do tauri — mesma restrição de `history_store.rs`); prova que a função não exige o
        // `AppHandle` que `modules/mod.rs` já resolveu antes de chamar.
        register_config_bound_tools(Arc::new(store), Some(dir.0.join("chat.db")));

        let all = crate::modules::tools::registry().get_all();
        for name in ["web", "content", "collaboration"] {
            let tool = all.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} registrada"));
            assert_eq!(tool.plugin_id, "mika.brain");
        }

        let executed = crate::modules::tools::registry()
            .execute("web", json!({ "action": "search", "query": "rust" }))
            .await
            .unwrap();
        assert_eq!(executed["errorCode"], json!("not_configured"), "deveria refletir browser.enabled=false do store injetado");
    }
}
