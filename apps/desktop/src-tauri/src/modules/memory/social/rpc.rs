//! `mika:plugin-rpc` de `mika.brain` — os 4 métodos `brain_*_social_fact`, porte de
//! `apps/desktop/src/main/services/brain/brain-social-rpc.ts::registerSocialFactRpc`.
//!
//! `plugin_id` medido, não presumido: `registerSocialFactRpc(ctx, brain)` é chamado de
//! `services/brain/index.ts` sobre o `MikaContext` cujo `manifest.id` é `'mika.brain'`
//! (`brain/index.ts:30`) — o mesmo id que `bootstrap/tools.ts::registerFeatureWithTools('mika.brain',
//! BrainFeature)` já usa para as tools deste plugin (`modules::brain::tools`, outro dono, outra
//! rodada).
//!
//! **Cada handler devolve o valor CRU**, sem envelope `{success,data}` — o TS também devolve: nenhum
//! dos 4 `ctx.rpc.handle(...)` do original constrói `ok()/fail()` (ao contrário de
//! `person_catalog::rpc`); `rpc::dispatch` embrulha uma única vez. `Err` aqui é o equivalente Rust de
//! uma promise rejeitada no TS — os dois casos abaixo replicam exatamente QUANDO o TS rejeita, não
//! quando ele resolve com um objeto de negócio, porque nenhum dos 4 handlers originais tem essa forma.
//!
//! ## Backend
//! [`SocialMemoryService`] (`super::mod`) já cobre os 3 primeiros byte a byte: `get_graph().facts`,
//! `add_fact`, `delete_fact`. Só `brain_update_social_fact` não tinha método de serviço — o TS FURA a
//! própria abstração (`(socialMemory as unknown as { store }).store`) para mexer direto na tabela
//! `social_facts`; aqui a mesma operação usa [`store::get_by_id`]/[`store::upsert`] direto, sem
//! inventar um método novo em `SocialMemoryService` para uma escrita que só este RPC faz.
//!
//! ## Divergências de forma medidas contra o TS
//! - `brain_add_social_fact`: TS não valida `fact` antes de usar — `input.fact.trim()` dentro de
//!   `addFact` estoura em `TypeError` se `fact` vier ausente, promise rejeitada, `handler-failed`. Aqui
//!   a ausência vira `Err("fact is required")` explicitamente — MESMO código de erro no transporte
//!   (`handler-failed`), mensagem nomeada em vez de um crash sem nome.
//! - `brain_add_social_fact` com `memory.enabled=false`: [`SocialMemoryService::add_fact`] devolve
//!   `Ok(None)` (ver o cabeçalho de `social/mod.rs`); o TS lança `MemoryRecordingDisabledError` sem
//!   capturar. Os dois caminhos terminam no mesmo lugar do lado do chamador — promise rejeitada /
//!   `Err` — então `Ok(None)` vira `Err` aqui para preservar esse destino.
//! - `brain_update_social_fact`, id não encontrado: o TS lança `Error('Fato não encontrado')` sem
//!   capturar → `handler-failed`. `rg` em `apps/desktop/src/renderer` não acha NENHUMA chamada a
//!   nenhum dos 4 métodos deste arquivo — não existe UI lendo `error.message` para decidir um fluxo de
//!   negócio hoje. Sem esse consumidor para justificar `Ok({success:false})` (o padrão que
//!   `person_catalog::rpc` usa quando a UI real lê `res.error` no caminho de SUCESSO), a leitura
//!   correta é reproduzir o mesmo canal de falha do TS ao pé da letra: `Err` → `handler-failed`.
//! - `brain_update_social_fact`, leitura do fato existente: o TS faz `store.getAll('social_facts')` +
//!   `.find(f => f.id === id)` (O(n) no histórico inteiro). Aqui é [`store::get_by_id`] (O(1) pela
//!   chave) — mesmo registro, mesma tabela, resultado observável idêntico; só o custo muda.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::memory::MemoryStore;
use crate::modules::plugins::rpc::{registry, RpcHandler};

use super::{AddFactInput, SocialMemoryService};

const PLUGIN_ID: &str = "mika.brain";

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

struct GetSocialFacts {
    mem: &'static MemoryStore,
}

#[async_trait]
impl RpcHandler for GetSocialFacts {
    /// = `ctx.rpc.handle('brain_get_social_facts', async () => (await socialMemory.getGraph()).facts)`.
    async fn call(&self, _args: Value) -> Result<Value, String> {
        let graph = SocialMemoryService::new(self.mem).get_graph()?;
        serde_json::to_value(graph.facts).map_err(|e| format!("serializar fatos: {e}"))
    }
}

struct AddSocialFact {
    mem: &'static MemoryStore,
}

#[async_trait]
impl RpcHandler for AddSocialFact {
    /// = `ctx.rpc.handle('brain_add_social_fact', ...) => socialMemory.addFact({subject,fact,confidence})`.
    /// Ver o cabeçalho do arquivo para os dois `Err` que espelham um `throw` não capturado no TS.
    async fn call(&self, args: Value) -> Result<Value, String> {
        let fact = args.get("fact").and_then(Value::as_str).ok_or("fact is required")?;
        let input = AddFactInput {
            // "eu"/"user" caem no subject canônico do usuário: sem isto viravam um balde que a persona nunca lê.
            subject: args.get("subject").and_then(Value::as_str).map(super::resolve_self_subject),
            fact: fact.to_string(),
            confidence: args.get("confidence").and_then(Value::as_f64),
            ..Default::default()
        };
        let recording_allowed = crate::modules::memory::recording_allowed_cached();
        match SocialMemoryService::new(self.mem).add_fact(recording_allowed, input, now_ms())? {
            Some(fact) => serde_json::to_value(fact).map_err(|e| format!("serializar fato: {e}")),
            None => Err("memory.enabled is false — social memory write was skipped, nothing was persisted".into()),
        }
    }
}

struct DeleteSocialFact {
    mem: &'static MemoryStore,
}

#[async_trait]
impl RpcHandler for DeleteSocialFact {
    /// = `ctx.rpc.handle('brain_delete_social_fact', ...) => ({ success: await socialMemory.deleteFact(id) })`.
    /// `id` ausente vira `""`, que nunca casa com um id gravado — mesmo destino de `deleteFact(undefined)`
    /// no TS (não encontra, devolve `false`), sem precisar de um `Err` nomeado à parte.
    async fn call(&self, args: Value) -> Result<Value, String> {
        let id = args.get("id").and_then(Value::as_str).unwrap_or("");
        let success = SocialMemoryService::new(self.mem).delete_fact(id)?;
        Ok(json!({ "success": success }))
    }
}

struct UpdateSocialFact {
    mem: &'static MemoryStore,
}

#[async_trait]
impl RpcHandler for UpdateSocialFact {
    /// = `ctx.rpc.handle('brain_update_social_fact', ...)`. Ver o cabeçalho do arquivo para a decisão
    /// de manter `Err` (não `Ok({success:false})`) quando `id` não é encontrado.
    async fn call(&self, args: Value) -> Result<Value, String> {
        let id = args.get("id").and_then(Value::as_str).unwrap_or("");
        let fact = args.get("fact").and_then(Value::as_str).ok_or("fact is required")?;
        let confidence = args.get("confidence").and_then(Value::as_f64);
        let recording_allowed = crate::modules::memory::recording_allowed_cached();
        // O fato editado ganha id novo (o id deriva do texto) e passa pela redação de segredos.
        match SocialMemoryService::new(self.mem).update_fact(recording_allowed, id, fact, confidence, now_ms())? {
            Some(updated) => serde_json::to_value(updated).map_err(|e| format!("serializar fato: {e}")),
            None => Err("Fato não encontrado".into()),
        }
    }
}

/// Chamado por `modules::register_config_bound_tools(app)` (fora deste diretório — reportado, não
/// aplicado; ver o relatório desta rodada) — mesma segunda passada que já dá `AppHandle` a
/// `person_catalog::rpc::register`. Resolve o MESMO `MemoryStore` de processo que
/// `modules::memory::social_reader()`/`episodic_reader()` usam (ver `modules/memory/mod.rs`), nunca
/// uma conexão dedicada.
pub(crate) fn register(app: &tauri::AppHandle) -> Result<(), String> {
    let mem = crate::modules::memory::memory_store(app)?;
    register_with_store(mem);
    Ok(())
}

/// Separado de [`register`] só para o teste alcançar sem `AppHandle` real — mesma necessidade que
/// `person_catalog::rpc::register_with_store` documenta.
fn register_with_store(mem: &'static MemoryStore) {
    let registry = registry();
    registry.register(PLUGIN_ID, "brain_get_social_facts", Arc::new(GetSocialFacts { mem }));
    registry.register(PLUGIN_ID, "brain_add_social_fact", Arc::new(AddSocialFact { mem }));
    registry.register(PLUGIN_ID, "brain_delete_social_fact", Arc::new(DeleteSocialFact { mem }));
    registry.register(PLUGIN_ID, "brain_update_social_fact", Arc::new(UpdateSocialFact { mem }));
}

/// Serializa os testes que chamam [`register_with_store`]: ele SOBRESCREVE as mesmas 4 chaves globais
/// de `plugins::rpc::registry()` a cada chamada, e `cargo test` roda em paralelo — mesmo padrão de
/// `gamer::rpc::TEST_LOCK`/`features::mood::TOOL_REGISTRATION_ORDER`.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
