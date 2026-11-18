//! Saldo pelas APIs oficiais: OpenRouter (`GET /api/v1/key`, reaproveita `llm::get_openrouter_quota`)
//! e ElevenLabs (`GET /v1/user/subscription`). Os parsers são puros e testados com corpos fixos; a
//! rede só existe em [`elevenlabs`], que passa pelo chokepoint de privacidade antes de sair.

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::schemas::provider_endpoints::ELEVENLABS_SUBSCRIPTION_URL;
use crate::config::store::ConfigStore;
use crate::modules::privacy;

const TIMEOUT: Duration = Duration::from_secs(8);

pub(super) fn ok(balance: Value) -> Value {
    json!({ "status": "ok", "balance": balance })
}

pub(super) fn failed(status: &str, error: &str) -> Value {
    json!({ "status": status, "error": error })
}

/// `quota` de `llm:getOpenRouterQuota` (USD) → saldo comum. Sem `limit` a chave não tem teto:
/// `limit`/`remaining` ficam `null` e a tela mostra só o gasto.
pub(super) fn from_openrouter(result: &Value) -> Value {
    let Some(q) = result.get("quota").filter(|_| result["ok"] == true) else {
        let error = result.get("error").and_then(Value::as_str).unwrap_or("OpenRouter sem resposta");
        let status = if error.contains("missing") { "missingKey" } else { "error" };
        return failed(status, error);
    };
    ok(json!({
        "unit": "usd",
        "used": q["usage"],
        "limit": q["limit"],
        "remaining": q["limitRemaining"],
        "resetsAt": q["limitReset"],
        "plan": if q["isFreeTier"] == true { json!("free") } else { Value::Null },
        "exhausted": q["exhausted"] == true,
    }))
}

/// Corpo de `/v1/user/subscription` → saldo em caracteres (a unidade que a ElevenLabs cobra).
pub(super) fn parse_elevenlabs(body: &Value) -> Option<Value> {
    let used = body.get("character_count")?.as_f64()?;
    let limit = body.get("character_limit")?.as_f64()?;
    let remaining = (limit - used).max(0.0);
    let reset_ms = body.get("next_character_count_reset_unix").and_then(Value::as_i64).map(|s| s * 1000);
    Some(json!({
        "unit": "characters",
        "used": used,
        "limit": limit,
        "remaining": remaining,
        "resetsAt": reset_ms,
        "plan": body.get("tier").and_then(Value::as_str),
        "exhausted": remaining <= 0.0,
    }))
}

/// Status HTTP → frase. A ElevenLabs devolve 401 também para chave válida sem a permissão
/// `user_read` (chave restrita a TTS) — o caso mais comum, então a frase diz como resolver.
fn elevenlabs_status(status: u16) -> String {
    match status {
        401 | 403 => "ElevenLabs recusou a consulta (401/403): a chave precisa da permissão \"User: Read\"".into(),
        429 => "ElevenLabs: limite de requisições (429), tente de novo em instantes".into(),
        other => format!("ElevenLabs respondeu HTTP {other}"),
    }
}

pub(super) async fn elevenlabs(store: &ConfigStore) -> Value {
    let key = store.get("apiKeys.elevenLabs").and_then(|v| v.as_str().map(|s| s.trim().to_string())).unwrap_or_default();
    if key.is_empty() {
        return failed("missingKey", "Chave da ElevenLabs não configurada");
    }
    if let Err(error) = privacy::assert_network_allowed(store, ELEVENLABS_SUBSCRIPTION_URL, "voice:elevenlabs") {
        return failed("error", &error);
    }
    // Só no binário `e2e` (dupla trava): a assinatura vem do dublê local, sem a nuvem nem chave real.
    let url = crate::modules::brain::llm_provider::e2e_cloud_base()
        .map_or_else(|| ELEVENLABS_SUBSCRIPTION_URL.to_string(), |base| format!("{base}/v1/user/subscription"));
    let response = match crate::modules::llm::http_client().get(url).header("xi-api-key", &key).timeout(TIMEOUT).send().await {
        Ok(r) => r,
        // Nunca repassa o erro do reqwest (carrega a URL).
        Err(_) => return failed("error", "ElevenLabs inacessível"),
    };
    if !response.status().is_success() {
        return failed("error", &elevenlabs_status(response.status().as_u16()));
    }
    match response.json::<Value>().await.ok().as_ref().and_then(parse_elevenlabs) {
        Some(balance) => ok(balance),
        None => failed("error", "ElevenLabs: resposta inválida"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevenlabs_subscription_becomes_characters_left_and_reset_in_ms() {
        let body = json!({ "tier": "starter", "character_count": 1000, "character_limit": 10000, "next_character_count_reset_unix": 1_738_356_858 });
        let b = parse_elevenlabs(&body).unwrap();
        assert_eq!(b["unit"], "characters");
        assert_eq!(b["remaining"], 9000.0);
        assert_eq!(b["resetsAt"], 1_738_356_858_000_i64);
        assert_eq!(b["plan"], "starter");
        assert_eq!(b["exhausted"], false);
    }

    #[test]
    fn elevenlabs_over_the_limit_is_exhausted_never_a_negative_balance() {
        let b = parse_elevenlabs(&json!({ "character_count": 10_500, "character_limit": 10_000 })).unwrap();
        assert_eq!(b["remaining"], 0.0);
        assert_eq!(b["exhausted"], true);
        assert_eq!(b["resetsAt"], Value::Null);
    }

    #[test]
    fn elevenlabs_body_without_the_counters_is_rejected_not_read_as_zero() {
        assert!(parse_elevenlabs(&json!({ "tier": "free" })).is_none());
    }

    #[test]
    fn elevenlabs_permission_error_says_which_permission_is_missing() {
        assert!(elevenlabs_status(401).contains("User: Read"));
    }

    #[test]
    fn openrouter_quota_maps_to_usd_balance_and_uncapped_key_keeps_null_limit() {
        let capped = json!({ "ok": true, "quota": { "usage": 2.5, "limit": 10.0, "limitRemaining": 7.5, "limitReset": "monthly", "isFreeTier": false, "exhausted": false } });
        let b = &from_openrouter(&capped)["balance"];
        assert_eq!((b["unit"].as_str(), b["remaining"].as_f64(), b["resetsAt"].as_str()), (Some("usd"), Some(7.5), Some("monthly")));

        let uncapped = json!({ "ok": true, "quota": { "usage": 1.0, "limit": null, "limitRemaining": null, "isFreeTier": true } });
        let b = &from_openrouter(&uncapped)["balance"];
        assert_eq!((b["limit"].is_null(), b["plan"].as_str()), (true, Some("free")));
    }

    #[test]
    fn openrouter_failure_keeps_its_reason_and_flags_a_missing_key() {
        let missing = from_openrouter(&json!({ "ok": false, "error": "OpenRouter API key is missing. Please configure it in Settings." }));
        assert_eq!(missing["status"], "missingKey");
        assert_eq!(from_openrouter(&json!({ "ok": false, "error": "OpenRouter unreachable" }))["status"], "error");
    }
}
