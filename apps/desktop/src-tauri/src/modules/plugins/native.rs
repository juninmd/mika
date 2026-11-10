//! `mika:plugin-rpc` servido em Rust — o canal que o cabeçalho de `mod.rs` passou dias recusando.
//!
//! **O que mudou não foi a análise, foi o denominador.** A recusa (ver `channel_gate.rs`) dizia:
//! `bridge.rs` roteia por canal INTEIRO, logo reivindicar com o registro vazio devolveria
//! `handler-missing` para os 25 métodos vivos — pior que o sidecar, que responde certo. Verdade
//! enquanto o registro estava vazio. Em 2026-09-22 os quatro donos foram portados e o registro
//! encheu: 24 dos 25 despacham de verdade. O 25º, `person_catalog_image`, ganhou handler em
//! 2026-09-28 (`features/person_catalog/image.rs`, fetch guardado por hop + MIME pelos bytes).
//!
//! Não há guarda a acrescentar aqui: `rpc::dispatch` já aplica `assert_caller_allowed` (superfície
//! + identidade da `plugin-view`) e valida `pluginId`/`method` contra o mesmo regex do
//! `pluginRpcRequestSchema`. Este arquivo é só o fio entre o canal e ela — por isso não tem lógica
//! própria, e é assim que tem de continuar.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::native::NativeModule;
use crate::protocol::InvokeContext;

const CH_PLUGIN_RPC: &str = "mika:plugin-rpc";

const CHANNELS: &[&str] = &[CH_PLUGIN_RPC];

pub struct PluginRpcModule;

#[async_trait]
impl NativeModule for PluginRpcModule {
    fn name(&self) -> &'static str {
        "plugin-rpc"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        _app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        match channel {
            // `Ok` mesmo quando a resposta é falha: o contrato (`pluginRpcResponseSchema`) é uma
            // UNIÃO, e `unwrapPluginRpcResponse` do outro lado espera o envelope resolvido para
            // transformar `error.code` em `Error`. Rejeitar o invoke apagaria o código de erro.
            CH_PLUGIN_RPC => Ok(super::rpc::dispatch(ctx.window_type, ctx.plugin_id.as_deref(), &payload).await),
            other => Err(format!("canal de plugin RPC desconhecido: {other}")),
        }
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
