//! Gate de aprovação por RISCO DA CHAMADA (nome + argumentos), compartilhado pelas duas portas de
//! entrada do registro — antes só o IPC `mika:tool-execute` o aplicava, e o loop de chat (o LLM)
//! chamava `execute()` direto: `memory_console clear_episodic`, `person_catalog delete`,
//! `user_profile forget`, `desktop launch` e `export_data` rodavam sem pergunta nenhuma.
//!
//! [`ToolOrigin`] é a distinção: `User` é o clique do usuário na UI (o IPC pergunta por
//! `is_high_risk_call` e já chega aqui aprovado), `Llm` é o modelo escolhendo a ação sozinho
//! (pergunta por `is_llm_gated_call`, a política mais larga), `System` é código do próprio app
//! (mood, atalhos) e nunca tem risco por ação.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{ToolRegistry, ToolOrigin};
use crate::config::store::ConfigStore;
use crate::modules::tools::access;
use crate::modules::tools::approval::{ApprovalRequest, ApprovalScope};

impl ToolRegistry {
    /// Guarda o `ConfigStore` para o gate honrar `brain.approveRiskyTools`/`brain.autoApproveTools`
    /// nas chamadas do LLM. Sem store (boot incompleto) o gate PERGUNTA — nunca libera.
    pub fn bind_config(&self, store: Arc<ConfigStore>) {
        *self.config.write().unwrap_or_else(|e| e.into_inner()) = Some(store);
    }

    fn bound_config(&self) -> Option<Arc<ConfigStore>> {
        self.config.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn call_needs_approval(origin: ToolOrigin, name: &str, args: &Value) -> bool {
        match origin {
            ToolOrigin::System => false,
            ToolOrigin::User => access::is_high_risk_call(name, args),
            ToolOrigin::Llm => access::is_llm_gated_call(name, args),
        }
    }

    /// Porte de `ensureBrokerApproval`. Devolve `Some(negação)` quando a chamada NÃO pode seguir.
    ///
    /// Tool marcada `destructive` fica de fora: [`ToolRegistry::execute_as`] a gateia logo depois, e
    /// dois diálogos para uma decisão só significam que o segundo chega depois de o primeiro já ter
    /// liberado. `brain.approveRiskyTools` desligado dispensa a pergunta (política do usuário) —
    /// exceto para URL suspeita de exfiltração, que pergunta sempre e nunca grava concessão
    /// permanente (um "sempre" aqui liberaria o canal de saída para toda URL futura).
    pub(crate) async fn ensure_call_approval(
        &self,
        origin: ToolOrigin,
        store: Option<&ConfigStore>,
        name: &str,
        args: &Value,
        call_id: &str,
    ) -> Option<Value> {
        if !Self::call_needs_approval(origin, name, args) || self.is_destructive_tool(name) {
            return None;
        }

        let sticky = !access::is_exfil_suspect_call(name, args);
        let enabled = store
            .and_then(|store| store.get("brain.approveRiskyTools"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let grant_key = access::approval_grant_key(name, args);
        let granted = self.session_grants.read().unwrap_or_else(|e| e.into_inner()).contains(&grant_key)
            || store
                .and_then(|store| store.get("brain.autoApproveTools"))
                .and_then(|value| value.as_array().cloned())
                .is_some_and(|granted| granted.iter().filter_map(Value::as_str).any(|key| key == grant_key));

        if sticky && (!enabled || granted) {
            return None;
        }

        let decision = self
            .request_approval(ApprovalRequest {
                tool_call_id: call_id.to_string(),
                tool_name: name.to_string(),
                args: args.clone(),
            })
            .await;

        if !decision.approved {
            // Nome da tool, nunca os argumentos: `user_profile`/`person_catalog` carregam PII, e um
            // log de recusa é justamente o lugar onde ela ficaria para sempre.
            log::warn!("tool call denied by the user: {name}");
            return Some(json!({ "success": false, "error": "user_denied" }));
        }

        if sticky {
            self.remember_grant(store, decision.scope, grant_key);
        }
        None
    }

    /// `Session` fica só na memória do registro; `Always` persiste em `brain.autoApproveTools`.
    fn remember_grant(&self, store: Option<&ConfigStore>, scope: ApprovalScope, grant_key: String) {
        match scope {
            ApprovalScope::Once => {}
            ApprovalScope::Session => {
                self.session_grants.write().unwrap_or_else(|e| e.into_inner()).insert(grant_key);
            }
            ApprovalScope::Always => {
                let Some(store) = store else { return };
                let mut granted = store
                    .get("brain.autoApproveTools")
                    .and_then(|value| value.as_array().cloned())
                    .unwrap_or_default();
                if !granted.iter().any(|key| key.as_str() == Some(grant_key.as_str())) {
                    granted.push(json!(grant_key));
                    let _ = store.set("brain.autoApproveTools", Value::Array(granted));
                }
            }
        }
    }

    /// A chamada não pode ser largada no meio pelo abort/teto do turno: tool `destructive` ou ação que
    /// o broker pergunta ao usuário (risco por AÇÃO). Largar um `clear_episodic` pela metade deixa efeito parcial.
    pub fn must_finish_once_started(&self, name: &str, args: &Value) -> bool {
        self.is_destructive_tool(name) || access::is_llm_gated_call(name, args)
    }

    /// Gate de [`ToolRegistry::execute_as`]: só o LLM é perguntado aqui (o IPC pergunta em
    /// `native.rs`, antes de emitir o HUD, e o sistema nunca é).
    pub(super) async fn gate_llm_call(&self, origin: ToolOrigin, name: &str, args: &Value) -> Option<Value> {
        if origin != ToolOrigin::Llm {
            return None;
        }
        let call_id = format!("llm-{name}-{}", uuid::Uuid::new_v4());
        let store = self.bound_config();
        self.ensure_call_approval(origin, store.as_deref(), name, args, &call_id).await
    }
}

#[cfg(test)]
#[path = "broker_tests.rs"]
mod tests;
