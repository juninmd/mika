//! `providerBilling:get` — créditos, cota e uso atual do provedor escolhido em cada tela de provider
//! (LLM, TTS, STT), para a pessoa saber se ele ainda consegue atender.
//!
//! Saldo vem da API oficial quando ela existe para a chave comum (OpenRouter, ElevenLabs — ver
//! [`catalog`]); quando não existe, `api.status = "unavailable"` e a tela diz "não disponível pela
//! API" ao lado do uso que a própria Mika mediu ([`usage`]). Número de saldo nunca é estimado.

mod catalog;
mod remote;
pub(crate) mod usage;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;
use catalog::{BalanceApi, Spec};

const CH_GET: &str = "providerBilling:get";
const CHANNELS: &[&str] = &[CH_GET];

async fn balance(store: &ConfigStore, spec: Spec) -> Value {
    match spec.api {
        BalanceApi::OpenRouter => remote::from_openrouter(&crate::modules::llm::get_openrouter_quota(store).await),
        BalanceApi::ElevenLabs => remote::elevenlabs(store).await,
        BalanceApi::None if spec.free => json!({ "status": "notApplicable" }),
        BalanceApi::None => json!({ "status": "unavailable" }),
    }
}

async fn local_usage(app: &AppHandle, provider: &str, spec: Spec) -> Result<Value, String> {
    let Some(meter) = spec.meter else { return Ok(Value::Null) };
    let (app, provider) = (app.clone(), provider.to_string());
    tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::modules::telemetry::token_usage_db(&app)?.lock().unwrap_or_else(|e| e.into_inner());
        usage::usage_with(&conn, &provider, meter)
    })
    .await
    .map_err(|e| format!("token_usage.db: {e}"))?
}

async fn get(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let provider = payload.get("provider").and_then(Value::as_str).unwrap_or_default();
    let spec = catalog::spec(provider).ok_or_else(|| format!("provedor desconhecido: '{provider}'"))?;
    let store = crate::state::config_store(app)?;
    let (api, usage) = tokio::join!(balance(&store, spec), local_usage(app, provider, spec));
    Ok(json!({ "provider": provider, "local": spec.local, "free": spec.free, "api": api, "usage": usage? }))
}

pub struct ProviderBillingModule;

#[async_trait]
impl NativeModule for ProviderBillingModule {
    fn name(&self) -> &'static str {
        "provider-billing"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }
        match channel {
            CH_GET => get(app, &payload).await,
            other => Err(format!("provider-billing: canal '{other}' declarado mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(ProviderBillingModule)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_channel_is_declared_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc.ts");
        assert!(contract.contains(&format!("channel: '{CH_GET}'")));
    }
}
