//! `ApprovalGateService` — port de `packages/sdk/src/tools/approval-gate.ts`.
//!
//! Só o gate em si: pedir aprovação, esperar um handler decidir, negar por padrão sem handler ou
//! em timeout. **Não é** o porte de `apps/desktop/src/main/ipc/tool-approval.ts` — aquela camada
//! escolhe a janela (`pickApprovalWindow`), manda o `ToolApprovalRequest` pelo IPC e usa o backstop
//! de UI (`TOOL_APPROVAL_BACKSTOP_MS` = 65s, `shared/tool-approval-constants.ts`). Essa camada
//! precisa de `AppHandle` para emitir evento numa janela — fora do que este arquivo pode fazer (ver
//! `registry.rs`: `ToolHandler::call` não recebe `AppHandle`, de propósito). O timeout AQUI é o
//! interno do próprio `ApprovalGateService` do lado TS (`TIMEOUT_MS = 30_000`, não o backstop de
//! UI): quem tiver o `AppHandle` primeiro instala um [`ApprovalHandler`] real (que emite pro
//! renderer e espera a resposta) via `ToolRegistry::set_approval_handler`; até lá, sem handler
//! configurado, todo pedido nega — mesmo fail-closed que `mcp/register.rs` já aplica hoje.
//!
//! ## Por que existe agora
//!
//! `modules/mcp/register.rs` registra tool de servidor de terceiro e nega TODA chamada, sempre,
//! hardcoded, porque não havia gate nenhum aqui. Isso continua seguro sem este arquivo. O que este
//! arquivo destrava: um caminho real para `ToolRegistry::execute` (`registry.rs`) checar
//! `destructive` e rotear pelo gate ANTES de despachar — testado ponta a ponta nos testes deste
//! módulo (`registry.rs::tests`), que são o consumidor real hoje. Trocar `mcp/register.rs` de
//! `.register(...)` para `.register_destructive(...)` (uma linha) é o próximo passo de quem for dono
//! daquele arquivo — fora do escopo desta rodada (só `modules/tools/` é meu).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::watch;

/// Mesmo `TIMEOUT_MS` de `approval-gate.ts` — o prazo interno do gate, não o backstop de UI.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalScope {
    Once,
    Session,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub approved: bool,
    pub scope: ApprovalScope,
}

const DENIED_ONCE: ApprovalDecision = ApprovalDecision { approved: false, scope: ApprovalScope::Once };

/// Handler real (janela + IPC) vive fora deste crate-por-enquanto; um `Err` aqui é tratado como o
/// handler síncrono que lança no lado TS — nunca escapa, sempre vira negação.
#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn decide(&self, request: ApprovalRequest) -> Result<ApprovalDecision, String>;
}

/// Port de `ApprovalGateService`. Um por `ToolRegistry` (mesma relação 1:1 de
/// `ToolRegistryService`/`ApprovalGateService` no TS), não um singleton próprio — `registry.rs`
/// mantém a instância.
pub struct ApprovalGateService {
    handler: RwLock<Option<Arc<dyn ApprovalHandler>>>,
    pending: std::sync::Mutex<HashMap<String, watch::Receiver<Option<ApprovalDecision>>>>,
    timeout: Duration,
}

impl ApprovalGateService {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
    }

    /// `timeout` custom só existe para teste: o prazo de produção é o `DEFAULT_TIMEOUT_MS` fixo
    /// acima (via [`Self::new`]); esperar 30s de verdade por um teste de timeout violaria a
    /// execução econômica que o AGENTS.md pede.
    fn with_timeout(timeout: Duration) -> Self {
        Self { handler: RwLock::new(None), pending: std::sync::Mutex::new(HashMap::new()), timeout }
    }

    pub fn set_handler(&self, handler: Arc<dyn ApprovalHandler>) {
        *self.handler.write().unwrap_or_else(|e| e.into_inner()) = Some(handler);
        log::info!("approval handler registered");
    }

    /// Mesma política de `requestApproval`: sem handler, nega sem nem perguntar; pedido duplicado
    /// (mesmo `tool_call_id`, ainda em voo) espera a MESMA decisão em vez de perguntar de novo.
    pub async fn request_approval(&self, request: ApprovalRequest) -> ApprovalDecision {
        let handler = { self.handler.read().unwrap_or_else(|e| e.into_inner()).clone() };
        let Some(handler) = handler else {
            log::warn!("no approval handler set, denying by default: tool={}", request.tool_name);
            return DENIED_ONCE;
        };

        // Check-and-insert precisa ser UMA seção crítica sob o mesmo lock: checar `existing` e só
        // depois inserir, em duas aquisições separadas, deixa uma janela onde dois pedidos
        // concorrentes com o mesmo `tool_call_id` passam ambos pelo "não existe" e cada um chama o
        // handler por conta própria (o segundo `insert` só sobrescreve o watch do primeiro no mapa,
        // sem religar quem já seguiu pelo caminho "New"). Já causou flake sob `cargo test --lib`
        // paralelo: o handler era chamado 2x em vez de 1x.
        let tx_for_new_request = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(rx) = pending.get(&request.tool_call_id) {
                Err(rx.clone())
            } else {
                let (tx, rx) = watch::channel(None);
                pending.insert(request.tool_call_id.clone(), rx);
                Ok(tx)
            }
        };

        let tx = match tx_for_new_request {
            Err(mut rx) => {
                log::warn!("approval request already pending: {}", request.tool_call_id);
                loop {
                    if let Some(decision) = *rx.borrow() {
                        return decision;
                    }
                    if rx.changed().await.is_err() {
                        return DENIED_ONCE;
                    }
                }
            }
            Ok(tx) => tx,
        };

        let decision = self.wait_for_decision(handler, &request).await;

        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.remove(&request.tool_call_id);
        }
        let _ = tx.send(Some(decision));

        decision
    }

    async fn wait_for_decision(&self, handler: Arc<dyn ApprovalHandler>, request: &ApprovalRequest) -> ApprovalDecision {
        let context = (request.tool_name.clone(), request.tool_call_id.clone());
        match tokio::time::timeout(self.timeout, handler.decide(request.clone())).await {
            Err(_) => {
                log::warn!("approval timed out, denying: tool={} id={}", context.0, context.1);
                DENIED_ONCE
            }
            Ok(Err(error)) => {
                log::warn!("approval handler failed, denying: tool={} id={} error={error}", context.0, context.1);
                DENIED_ONCE
            }
            Ok(Ok(decision)) => decision,
        }
    }

    /// Port de `clearPending` — usado por quem fecha a janela que perguntava (fora do escopo
    /// desta rodada), e pelos próprios testes deste arquivo para isolar estado entre casos.
    #[cfg(test)]
    pub fn clear_pending(&self) {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

impl Default for ApprovalGateService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn req(id: &str) -> ApprovalRequest {
        ApprovalRequest { tool_call_id: id.into(), tool_name: "steam".into(), args: Value::Null }
    }

    struct Approves;
    #[async_trait]
    impl ApprovalHandler for Approves {
        async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
            Ok(ApprovalDecision { approved: true, scope: ApprovalScope::Once })
        }
    }

    struct Denies;
    #[async_trait]
    impl ApprovalHandler for Denies {
        async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
            Ok(ApprovalDecision { approved: false, scope: ApprovalScope::Once })
        }
    }

    struct Fails;
    #[async_trait]
    impl ApprovalHandler for Fails {
        async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
            Err("dialog crashed".into())
        }
    }

    /// O caso que mais importa para o veto do SecOps: sem handler configurado, nega sem nem
    /// perguntar — mesmo comportamento de `mcp/register.rs` hoje, só que agora vindo de um gate de
    /// verdade em vez de um `Err` fixo no handler da tool.
    #[tokio::test]
    async fn denies_by_default_without_a_handler() {
        let gate = ApprovalGateService::new();
        let decision = gate.request_approval(req("t1")).await;
        assert!(!decision.approved);
        assert_eq!(decision.scope, ApprovalScope::Once);
    }

    #[tokio::test]
    async fn approves_when_the_handler_approves() {
        let gate = ApprovalGateService::new();
        gate.set_handler(Arc::new(Approves));
        let decision = gate.request_approval(req("t2")).await;
        assert!(decision.approved);
    }

    #[tokio::test]
    async fn denies_when_the_handler_denies() {
        let gate = ApprovalGateService::new();
        gate.set_handler(Arc::new(Denies));
        let decision = gate.request_approval(req("t3")).await;
        assert!(!decision.approved);
    }

    /// Handler síncrono lançando no TS não escapa pro loop de ferramentas — aqui, `Err` do handler
    /// tem a mesma garantia: nunca propaga, sempre vira negação.
    #[tokio::test]
    async fn denies_when_the_handler_errors_instead_of_propagating() {
        let gate = ApprovalGateService::new();
        gate.set_handler(Arc::new(Fails));
        let decision = gate.request_approval(req("t4")).await;
        assert!(!decision.approved);
    }

    struct NeverResponds;
    #[async_trait]
    impl ApprovalHandler for NeverResponds {
        async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
            std::future::pending::<()>().await;
            unreachable!()
        }
    }

    #[tokio::test]
    async fn denies_on_timeout_instead_of_hanging_forever() {
        let gate = ApprovalGateService::with_timeout(Duration::from_millis(20));
        gate.set_handler(Arc::new(NeverResponds));
        let decision = gate.request_approval(req("t5")).await;
        assert!(!decision.approved);
    }

    struct CountsCalls {
        calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl ApprovalHandler for CountsCalls {
        async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok(ApprovalDecision { approved: true, scope: ApprovalScope::Once })
        }
    }

    /// Pedido duplicado com o MESMO `tool_call_id` em voo não pergunta duas vezes — dedupe pelo
    /// `Map` pendente, igual ao TS. Sem `sleep` entre os dois `spawn`: a correção do
    /// check-and-insert (uma seção crítica sob o mesmo lock em `request_approval`) garante dedupe
    /// não importa a ordem de escalonamento das duas tasks — antes da correção, este teste só
    /// passava com um `sleep(5ms)` entre os spawns dando tempo de `a` inserir antes de `b` checar;
    /// sob `cargo test --lib` paralelo (CPU disputada) essa janela de 5ms não era garantia
    /// suficiente e o handler era chamado 2x, expondo a mesma race que existia em produção.
    #[tokio::test]
    async fn concurrent_requests_with_the_same_tool_call_id_share_one_decision() {
        let gate = Arc::new(ApprovalGateService::new());
        let calls = Arc::new(AtomicUsize::new(0));
        gate.set_handler(Arc::new(CountsCalls { calls: calls.clone() }));

        let a = { let gate = gate.clone(); tokio::spawn(async move { gate.request_approval(req("dup")).await }) };
        let b = { let gate = gate.clone(); tokio::spawn(async move { gate.request_approval(req("dup")).await }) };

        let (decision_a, decision_b) = (a.await.unwrap(), b.await.unwrap());
        assert!(decision_a.approved);
        assert!(decision_b.approved);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "handler deveria ter sido chamado só uma vez");
    }

    #[tokio::test]
    async fn clear_pending_drops_in_flight_bookkeeping() {
        let gate = ApprovalGateService::new();
        {
            let mut pending = gate.pending.lock().unwrap();
            let (_tx, rx) = watch::channel(None);
            pending.insert("stale".into(), rx);
        }
        gate.clear_pending();
        assert!(gate.pending.lock().unwrap().is_empty());
    }
}
