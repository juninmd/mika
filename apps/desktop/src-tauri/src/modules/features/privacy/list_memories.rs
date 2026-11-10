//! Tool `list_memories` (LLM, leitura) — port de `apps/desktop/src/features/privacy/tools/index.ts`
//! (`list_memories`).
//!
//! Usa `EpisodicMemoryService::get_all` (ordem, TTL, poda de índice órfão — ver o cabeçalho de
//! `../../memory/episodic/mod.rs`) via `crate::modules::memory::episodic_reader()`: a porta estreita
//! que aquele módulo abriu nesta rodada para não duplicar a leitura estruturada do submódulo PRIVADO
//! `crate::modules::memory::episodic` (o risco que a versão anterior deste arquivo recusava correr —
//! ver histórico do arquivo). Diferente de `clear_history` (`clear.rs`), que só faz `DELETE` sobre um
//! glob de chaves sem interpretar dado nenhum.
//!
//! `total`/`memories` espelham `list_memories.execute` no TS: `total` é o tamanho de `getAll()`
//! ANTES do corte por `limit` (paginação real, não o total pós-corte), e cada entrada só leva
//! `id`/`content`/`type`/`timestamp` — mesmo recorte que `memoryEntrySchema.safeParse` faz lá
//! (`metadata`/`embedding` nunca saem desta tool).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::memory::episodic::EpisodicMemoryService;
use crate::modules::tools::{ToolDefinition, ToolHandler};

const DESCRIPTION: &str = "List the most recent episodic memories — what happened in past interactions, not profile facts like name or preferences (that is user_profile). Use when the user asks what you remember about recent conversations.";

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "list_memories".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum number of memories to return, most recent first.",
                },
            },
            "required": [],
        }),
        plugin_id: "mika.privacy".into(),
        requires_window: false,
    }
}

/// = `list_memories.execute`, injetável: `run()` é o único ponto que resolve
/// `episodic_reader()` (o singleton de processo — nenhum teste de unidade consegue popular o
/// `OnceLock` por trás dele, precisa de `AppHandle`). `total` é o tamanho de `getAll()` ANTES do
/// corte por `limit` — mesma paginação do TS (`ok({ total: all.length, memories })`), nunca o total
/// pós-corte. Cada entrada só leva `id`/`content`/`type`/`timestamp`, o mesmo recorte que
/// `memoryEntrySchema.safeParse` fazia no TS (metadata/embedding nunca saem desta tool).
fn run(mem: Option<&EpisodicMemoryService>, args: &Value) -> Value {
    let Some(mem) = mem else {
        return json!({
            "success": false,
            "error": "memória episódica indisponível — o MemoryStore de processo ainda não abriu",
            "errorCode": "unavailable",
        });
    };
    let limit = args["limit"].as_u64().map(|n| n as usize).unwrap_or(20).clamp(1, 50);

    match mem.get_all() {
        Ok(all) => {
            let total = all.len();
            // `get_all()` já devolve mais recente primeiro (ver `episodic::queries::get_all`) — só
            // corta no limite, sem reordenar (paginação real, sem N+1: uma leitura, um corte).
            let memories: Vec<Value> = all
                .into_iter()
                .take(limit)
                .map(|entry| json!({ "id": entry.id, "content": entry.content, "type": entry.r#type, "timestamp": entry.timestamp }))
                .collect();
            json!({ "success": true, "data": { "total": total, "memories": memories } })
        }
        Err(error) => json!({
            "success": false,
            "error": format!("list_memories falhou: {error}"),
            "errorCode": "internal",
        }),
    }
}

struct ListMemoriesToolHandler;

#[async_trait]
impl ToolHandler for ListMemoriesToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(run(crate::modules::memory::episodic_reader().as_ref(), &args))
    }
}

/// Chamado por `super::register`. Sem `ConfigStore`: a resposta não depende de config nenhuma.
pub(crate) fn register() {
    crate::modules::tools::registry().register(tool_definition(), Arc::new(ListMemoriesToolHandler));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> crate::modules::memory::MemoryStore {
        crate::modules::memory::MemoryStore::open_in_memory().unwrap()
    }

    #[tokio::test]
    async fn degrades_honestly_when_the_process_store_never_opened() {
        // `ListMemoriesToolHandler::call` sempre resolve o singleton de processo, que nenhum teste
        // de unidade consegue popular (precisa de `AppHandle`) — o mesmo fallback honesto que
        // `memory_console` documenta.
        let handler = ListMemoriesToolHandler;
        let result = handler.call(json!({ "limit": 20 })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
        assert!(result.get("data").is_none(), "nunca devolve uma lista de memórias inventada");
    }

    #[test]
    fn lists_the_most_recent_memories_first_and_reports_the_pre_limit_total() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "mais antiga", None, None).unwrap();
        svc.store(true, "mais recente", None, None).unwrap();

        let result = run(Some(&svc), &json!({ "limit": 1 }));
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["total"], json!(2), "total é ANTES do corte por limit");
        let memories = result["data"]["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 1, "memories é DEPOIS do corte");
        assert_eq!(memories[0]["content"], json!("mais recente"));
    }

    #[test]
    fn entries_only_carry_id_content_type_and_timestamp_never_metadata_or_embedding() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "com metadata", None, Some(&json!({ "important": true }))).unwrap();

        let result = run(Some(&svc), &json!({}));
        let entry = &result["data"]["memories"][0];
        let keys: std::collections::BTreeSet<&str> = entry.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, std::collections::BTreeSet::from(["id", "content", "type", "timestamp"]));
    }

    #[test]
    fn limit_defaults_to_twenty_like_the_zod_schema() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        for i in 0..25 {
            svc.store(true, &format!("fato {i}"), None, None).unwrap();
        }

        let result = run(Some(&svc), &json!({}));
        assert_eq!(result["data"]["memories"].as_array().unwrap().len(), 20);
    }

    #[test]
    fn register_adds_list_memories_with_the_manifest_plugin_id() {
        register();
        let definition = crate::modules::tools::registry().get("list_memories").expect("list_memories não registrada");
        assert_eq!(definition.plugin_id, "mika.privacy");
    }
}
