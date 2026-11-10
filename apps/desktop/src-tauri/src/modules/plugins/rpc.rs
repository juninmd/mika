//! `mika:plugin-rpc` — o registro de handlers por plugin e a guarda de quem pode chamar.
//!
//! Porte de `main/infrastructure/bootstrap/plugin-rpc-registry.ts::dispatchPluginRpc` e do contrato
//! de `shared/plugin-rpc-contract.ts`.
//!
//! **Este arquivo NÃO reivindica o canal.** Ele é o esqueleto: cada feature portada registra os
//! próprios métodos em [`registry`], e o canal só pode ser reivindicado quando os 25 métodos que o
//! TypeScript serve hoje tiverem contraparte. O motivo é o de sempre nesta migração e está medido:
//! `bridge.rs` roteia por canal INTEIRO (`state.native.owns(&channel)`), nunca por método — então
//! reivindicar com 10 dos 25 portados não degrada 15 métodos, **quebra** os 15, que hoje funcionam.
//!
//! O inverso também é verdade e é o que justifica portá-los: enquanto este canal descer para o
//! Node, o sidecar inteiro continua vivo. Os 25 métodos não são um argumento para adiar — são a
//! lista de trabalho.
//!
//! ## Os 25 métodos, medidos por `rg "\.rpc\.handle\("` em 2026-09-22
//!
//! | Origem TS | Métodos | Serviço Rust que já existe |
//! |---|---|---|
//! | `features/person-catalog/handlers/person-catalog-rpc.ts` | 10 | `modules/features/person_catalog/` |
//! | `features/gamer-companion/handlers/gamer-companion-rpc.ts` | 7 | `modules/gamer/` |
//! | `features/mood/index.ts` | 4 | `modules/features/mood/` |
//! | `main/services/brain/brain-social-rpc.ts` | 4 | `modules/memory/social/` |
//!
//! Nenhum deles espera serviço novo: os quatro donos já estão portados. É plumbing, e é por isso
//! que a lista é trabalho e não bloqueio.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::protocol::WindowType;

/// Espelha `PluginRpcErrorCode` (`shared/plugin-rpc-contract.ts`). Os literais são o contrato: o
/// renderer desembrulha por `error.code` (`unwrapPluginRpcResponse`), e um código inventado aqui
/// chegaria como erro genérico sem a UI saber distinguir "não existe" de "não subiu".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcErrorCode {
    InvalidRequest,
    Unauthorized,
    HandlerMissing,
    HandlerFailed,
}

impl RpcErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid-request",
            Self::Unauthorized => "unauthorized",
            Self::HandlerMissing => "handler-missing",
            Self::HandlerFailed => "handler-failed",
        }
    }
}

pub fn failure(code: RpcErrorCode, message: impl Into<String>) -> Value {
    json!({ "success": false, "error": { "code": code.as_str(), "message": message.into() } })
}

pub fn success(data: Value) -> Value {
    json!({ "success": true, "data": data })
}

/// Um método RPC de plugin. `Err` do handler vira `handler-failed`, nunca pânico — mesma política
/// do `try/catch` em `dispatchPluginRpc`.
#[async_trait]
pub trait RpcHandler: Send + Sync {
    async fn call(&self, args: Value) -> Result<Value, String>;
}

/// Registro `pluginId → método → handler`. Um por processo, mesmo padrão de `tools::registry()`.
#[derive(Default)]
pub struct RpcRegistry {
    handlers: RwLock<HashMap<(String, String), Arc<dyn RpcHandler>>>,
}

impl RpcRegistry {
    /// Porte de `registerPluginRpcHandler`. O TS valida `pluginId`/`method` na hora do REGISTRO
    /// (`createPluginRpcRequest` lança), e não só na chamada — um id fora do padrão
    /// `mika.<algo>` nunca seria alcançável, e falhar no registro é o que torna isso visível.
    pub fn register(&self, plugin_id: &str, method: &str, handler: Arc<dyn RpcHandler>) {
        if !is_valid_plugin_id(plugin_id) || !is_valid_method(method) {
            log::error!("plugin RPC handler with an invalid identity was not registered: {plugin_id}:{method}");
            return;
        }
        let mut handlers = self.handlers.write().unwrap_or_else(|e| e.into_inner());
        if handlers.insert((plugin_id.to_string(), method.to_string()), handler).is_some() {
            log::warn!("plugin RPC handler replaced: {plugin_id}:{method}");
        }
    }

    fn get(&self, plugin_id: &str, method: &str) -> Option<Arc<dyn RpcHandler>> {
        let handlers = self.handlers.read().unwrap_or_else(|e| e.into_inner());
        handlers.get(&(plugin_id.to_string(), method.to_string())).cloned()
    }
}

static REGISTRY: OnceLock<RpcRegistry> = OnceLock::new();

pub fn registry() -> &'static RpcRegistry {
    REGISTRY.get_or_init(RpcRegistry::default)
}

/// Espelha o regex `^mika\.[a-z0-9.-]+$` de `pluginRpcRequestSchema`.
fn is_valid_plugin_id(plugin_id: &str) -> bool {
    plugin_id.len() <= 120
        && plugin_id.starts_with("mika.")
        && plugin_id.len() > "mika.".len()
        && plugin_id["mika.".len()..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-'))
}

/// Espelha o regex `^[A-Za-z0-9._:-]+$`.
fn is_valid_method(method: &str) -> bool {
    !method.is_empty()
        && method.len() <= 120
        && method.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// Porte de `PLUGIN_RPC_ALLOWED_WINDOW_TYPES` + a regra de identidade de `plugin-view`.
///
/// A segunda metade é a que importa: uma `plugin-view` só chama o PRÓPRIO `pluginId`. Sem ela, uma
/// view de plugin qualquer chamaria `person_catalog_delete` ou `brain_delete_social_fact` — as
/// janelas compartilham o mesmo preload, então a identidade é a única coisa que as separa.
pub fn assert_caller_allowed(window: WindowType, caller_plugin_id: Option<&str>, requested_plugin_id: &str) -> Result<(), Value> {
    let allowed = matches!(window, WindowType::Main | WindowType::Settings | WindowType::PluginView);
    if !allowed {
        // O log NÃO carrega `args`: métodos de catálogo de pessoas e de fatos sociais levam PII.
        log::warn!("plugin RPC unauthorized: window={window:?} requested={requested_plugin_id}");
        return Err(failure(RpcErrorCode::Unauthorized, "Plugin RPC caller is not allowed"));
    }

    if window == WindowType::PluginView {
        match caller_plugin_id.filter(|id| !id.is_empty()) {
            Some(caller) if caller == requested_plugin_id => {}
            _ => {
                log::warn!(
                    "plugin RPC unauthorized: plugin-view caller={:?} requested={requested_plugin_id}",
                    caller_plugin_id
                );
                return Err(failure(RpcErrorCode::Unauthorized, "Plugin RPC caller is not allowed"));
            }
        }
    }

    Ok(())
}

/// Porte de `dispatchPluginRpc`. Guarda → handler → resultado, com o mesmo código de erro em cada
/// ramo. Nunca `Err` de IPC: toda falha é uma RESPOSTA `{ success: false, error: { code } }`,
/// porque é assim que `unwrapPluginRpcResponse` a lê do outro lado.
pub async fn dispatch(window: WindowType, caller_plugin_id: Option<&str>, payload: &Value) -> Value {
    let plugin_id = payload.get("pluginId").and_then(Value::as_str).unwrap_or_default();
    let method = payload.get("method").and_then(Value::as_str).unwrap_or_default();

    if !is_valid_plugin_id(plugin_id) || !is_valid_method(method) {
        return failure(RpcErrorCode::InvalidRequest, "Plugin RPC request is malformed");
    }

    if let Err(denial) = assert_caller_allowed(window, caller_plugin_id, plugin_id) {
        return denial;
    }

    let Some(handler) = registry().get(plugin_id, method) else {
        // `HandlerMissing` e não `plugin-not-ready`: em Rust os handlers nascem no boot, não depois
        // de um carregamento assíncrono de plugin. `plugin-not-ready` existe no contrato para o
        // mundo do Node, onde `bootstrapStatus.pluginsReady` podia ainda ser falso — declarar a
        // variante sem esse estado seria código morto com nome de diagnóstico, por isso não existe.
        return failure(RpcErrorCode::HandlerMissing, format!("Plugin RPC unavailable for {plugin_id}:{method}"));
    };

    let args = payload.get("args").cloned().unwrap_or(Value::Null);
    match handler.call(args).await {
        Ok(data) => success(data),
        Err(error) => {
            log::error!("plugin RPC handler failed: {plugin_id}:{method}");
            failure(RpcErrorCode::HandlerFailed, error)
        }
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
