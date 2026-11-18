//! Uso medido localmente por provedor, em `token_usage.db`: tokens de LLM vêm da quebra que
//! `token_usage` já grava (`token_usage_by_provider`); caracteres de TTS e segundos de STT, de
//! `provider_usage` (gravado aqui, a cada síntese/transcrição paga que deu certo).

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::catalog::Meter;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub(super) fn record_with(conn: &Connection, provider: &str, amount: f64, now_ms: i64) -> Result<(), String> {
    if !amount.is_finite() || amount < 0.0 {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO provider_usage (provider, amount, requests, first_used_at, last_used_at) VALUES (?1, ?2, 1, ?3, ?3) \
         ON CONFLICT(provider) DO UPDATE SET amount = amount + ?2, requests = requests + 1, last_used_at = ?3",
        params![provider, amount, now_ms],
    )
    .map_err(|e| format!("falha ao gravar provider_usage: {e}"))?;
    Ok(())
}

/// Grava sem bloquear quem chamou (a fala/transcrição já está pronta para o usuário). Sem
/// `token_usage::arm` (testes de unidade) não grava nada.
pub(crate) fn record(provider: &'static str, amount: f64) {
    let Some(app) = crate::modules::token_usage::armed_app() else { return };
    let (app, now) = (app.clone(), now_ms());
    drop(tauri::async_runtime::spawn_blocking(move || {
        let result = crate::modules::telemetry::token_usage_db(&app).and_then(|db| {
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            record_with(&conn, provider, amount, now)
        });
        if let Err(error) = result {
            log::warn!("uso de {provider} não registrado: {error}");
        }
    }));
}

/// Segundos de áudio PCM16 mono (o único formato que o backend aceita, ver AGENTS.md §2.5).
pub(crate) fn pcm16_seconds(bytes: usize, sample_rate: u32) -> f64 {
    if sample_rate == 0 { 0.0 } else { bytes as f64 / 2.0 / f64::from(sample_rate) }
}

fn tokens(conn: &Connection, provider: &str) -> rusqlite::Result<Value> {
    conn.query_row(
        "SELECT COALESCE(SUM(total_tokens), 0), COALESCE(SUM(total_interactions), 0), COALESCE(SUM(total_cost_usd), 0), \
            COALESCE(SUM(unknown_cost_turns), 0), COALESCE(MAX(last_used_at), 0) FROM token_usage_by_provider WHERE provider = ?1",
        params![provider],
        |r| {
            let (requests, unknown): (i64, i64) = (r.get(1)?, r.get(3)?);
            // Custo só é um total quando todo turno teve preço; com algum desconhecido vira `null`
            // (somar zero no lugar seria um piso disfarçado de total, ver `token_usage.rs`).
            let cost = if unknown == 0 { json!(r.get::<_, f64>(2)?) } else { Value::Null };
            Ok(json!({ "amount": r.get::<_, i64>(0)?, "requests": requests, "costUsd": cost, "unknownCostRequests": unknown, "lastUsedAt": r.get::<_, i64>(4)? }))
        },
    )
}

fn metered(conn: &Connection, provider: &str) -> rusqlite::Result<Value> {
    let row = conn.query_row(
        "SELECT amount, requests, last_used_at FROM provider_usage WHERE provider = ?1",
        params![provider],
        |r| Ok(json!({ "amount": r.get::<_, f64>(0)?, "requests": r.get::<_, i64>(1)?, "costUsd": null, "unknownCostRequests": 0, "lastUsedAt": r.get::<_, i64>(2)? })),
    );
    match row {
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(json!({ "amount": 0, "requests": 0, "costUsd": null, "unknownCostRequests": 0, "lastUsedAt": 0 })),
        other => other,
    }
}

pub(super) fn usage_with(conn: &Connection, provider: &str, meter: Meter) -> Result<Value, String> {
    let mut usage = match meter {
        Meter::Tokens => tokens(conn, provider),
        Meter::Characters | Meter::Seconds => metered(conn, provider),
    }
    .map_err(|e| format!("falha ao ler uso de {provider}: {e}"))?;
    usage["unit"] = json!(meter.as_str());
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::telemetry::create_token_usage_table(&conn).unwrap();
        conn
    }

    #[test]
    fn tts_characters_accumulate_per_provider_without_mixing_providers() {
        let conn = conn();
        record_with(&conn, "elevenlabs", 120.0, 1_000).unwrap();
        record_with(&conn, "elevenlabs", 30.0, 2_000).unwrap();
        record_with(&conn, "azure-tts", 7.0, 3_000).unwrap();
        let u = usage_with(&conn, "elevenlabs", Meter::Characters).unwrap();
        assert_eq!((u["amount"].as_f64(), u["requests"].as_i64(), u["lastUsedAt"].as_i64()), (Some(150.0), Some(2), Some(2_000)));
        assert_eq!(u["unit"], "characters");
    }

    #[test]
    fn a_provider_never_used_reads_zero_not_an_error() {
        let u = usage_with(&conn(), "groq-whisper", Meter::Seconds).unwrap();
        assert_eq!((u["amount"].as_f64(), u["requests"].as_i64()), (Some(0.0), Some(0)));
    }

    #[test]
    fn llm_tokens_sum_every_surface_and_model_and_unknown_price_voids_the_cost_total() {
        let conn = conn();
        let insert = "INSERT INTO token_usage_by_provider (surface, provider, model, total_tokens, total_cost_usd, unknown_cost_turns, total_interactions, last_used_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)";
        conn.execute(insert, params!["chat", "openrouter", "a", 100, 0.002, 0, 10]).unwrap();
        conn.execute(insert, params!["vision", "openrouter", "b", 50, 0.001, 0, 20]).unwrap();
        conn.execute(insert, params!["chat", "google", "g", 70, 0.0, 1, 30]).unwrap();
        let or = usage_with(&conn, "openrouter", Meter::Tokens).unwrap();
        assert_eq!((or["amount"].as_i64(), or["requests"].as_i64(), or["lastUsedAt"].as_i64()), (Some(150), Some(2), Some(20)));
        assert!((or["costUsd"].as_f64().unwrap() - 0.003).abs() < 1e-12);
        assert_eq!(usage_with(&conn, "google", Meter::Tokens).unwrap()["costUsd"], Value::Null);
    }

    #[test]
    fn garbage_amounts_never_poison_the_counter() {
        let conn = conn();
        record_with(&conn, "openai-tts", f64::NAN, 1).unwrap();
        record_with(&conn, "openai-tts", -5.0, 1).unwrap();
        assert_eq!(usage_with(&conn, "openai-tts", Meter::Characters).unwrap()["requests"], 0);
    }

    #[test]
    fn pcm16_seconds_counts_two_bytes_per_sample() {
        assert_eq!(pcm16_seconds(32_000, 16_000), 1.0);
        assert_eq!(pcm16_seconds(100, 0), 0.0);
    }
}
