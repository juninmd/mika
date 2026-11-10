//! Acumula o uso de tokens de cada turno do chat em `token_usage.db` — porte de
//! `nextTokenUsageStats` (`chat-token-usage.ts`). Sem isto o painel de uso lia uma tabela que
//! ninguém escrevia e mostrava zero para sempre.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use rusqlite::{params, Connection};
use serde_json::Value;
use tauri::AppHandle;

/// Preço por token (USD) de entrada e saída de cada modelo do OpenRouter, alimentado pelo catálogo
/// vivo (`llm::fetch_openrouter_models`). Google e endpoint OpenAI-compatível não publicam preço por
/// API: ficam desconhecidos, nunca chutados (tabela estática de preço já apodreceu aqui uma vez).
static OPENROUTER_PRICES: LazyLock<Mutex<HashMap<String, (f64, f64)>>> = LazyLock::new(Mutex::default);
static LAST_PRICE_REFRESH_MS: AtomicI64 = AtomicI64::new(0);
/// Para quem gasta token sem `AppHandle` no ponto de chamada (embedding, ver [`record_embedding`]).
static APP: OnceLock<AppHandle> = OnceLock::new();
const PRICE_REFRESH_INTERVAL_MS: i64 = 10 * 60 * 1000;

/// `cost_usd: None` é preço desconhecido: conta o turno em `unknown_cost_turns` em vez de somar
/// zero — somar zero viraria um piso disfarçado de total.
fn record_with(conn: &Connection, prompt: u64, completion: u64, total: u64, cost_usd: Option<f64>, now_ms: i64) -> Result<(), String> {
    let total = if total == 0 { prompt + completion } else { total };
    let cost = cost_usd.filter(|c| c.is_finite()).unwrap_or(0.0);
    let unknown = i64::from(cost_usd.is_none());
    conn.execute(
        "INSERT INTO token_usage (id, total_tokens, total_prompt_tokens, total_completion_tokens, \
            total_cost_usd, unknown_cost_turns, total_interactions, first_used_at, last_used_at) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, 1, ?6, ?6) \
         ON CONFLICT(id) DO UPDATE SET \
            total_tokens = total_tokens + ?1, total_prompt_tokens = total_prompt_tokens + ?2, \
            total_completion_tokens = total_completion_tokens + ?3, total_cost_usd = total_cost_usd + ?4, \
            unknown_cost_turns = unknown_cost_turns + ?5, total_interactions = total_interactions + 1, \
            first_used_at = CASE WHEN first_used_at = 0 THEN ?6 ELSE first_used_at END, last_used_at = ?6",
        params![total as i64, prompt as i64, completion as i64, cost, unknown, now_ms],
    )
    .map_err(|e| format!("falha ao gravar token_usage: {e}"))?;
    Ok(())
}

/// Mesmo turno, quebrado por `surface`+`provider`+`model` (`token_usage_by_provider`, aditiva —
/// ver o cabeçalho de `create_token_usage_table` em `telemetry.rs`). Total e quebra nunca divergem
/// porque as duas gravações partem do mesmo `usage`/`cost_usd`/`now_ms` desta chamada.
fn record_by_provider_with(
    conn: &Connection,
    surface: &str,
    provider: &str,
    model: &str,
    prompt: u64,
    completion: u64,
    total: u64,
    cost_usd: Option<f64>,
    now_ms: i64,
) -> Result<(), String> {
    let total = if total == 0 { prompt + completion } else { total };
    let cost = cost_usd.filter(|c| c.is_finite()).unwrap_or(0.0);
    let unknown = i64::from(cost_usd.is_none());
    conn.execute(
        "INSERT INTO token_usage_by_provider (surface, provider, model, total_tokens, total_prompt_tokens, \
            total_completion_tokens, total_cost_usd, unknown_cost_turns, total_interactions, first_used_at, last_used_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9) \
         ON CONFLICT(surface, provider, model) DO UPDATE SET \
            total_tokens = total_tokens + ?4, total_prompt_tokens = total_prompt_tokens + ?5, \
            total_completion_tokens = total_completion_tokens + ?6, total_cost_usd = total_cost_usd + ?7, \
            unknown_cost_turns = unknown_cost_turns + ?8, total_interactions = total_interactions + 1, \
            first_used_at = CASE WHEN first_used_at = 0 THEN ?9 ELSE first_used_at END, last_used_at = ?9",
        params![surface, provider, model, total as i64, prompt as i64, completion as i64, cost, unknown, now_ms],
    )
    .map_err(|e| format!("falha ao gravar token_usage_by_provider: {e}"))?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Guarda os preços de um catálogo do OpenRouter já normalizado (`promptPrice`/`completionPrice`
/// como string decimal por token). Modelo sem os dois preços não entra.
pub(crate) fn remember_openrouter_prices(models: &[Value]) {
    let price = |m: &Value, key: &str| m.get(key)?.as_str()?.parse::<f64>().ok().filter(|p| p.is_finite() && *p >= 0.0);
    let Ok(mut prices) = OPENROUTER_PRICES.lock() else { return };
    for m in models {
        if let (Some(id), Some(input), Some(output)) = (m.get("id").and_then(Value::as_str), price(m, "promptPrice"), price(m, "completionPrice")) {
            prices.insert(id.to_string(), (input, output));
        }
    }
}

/// Pura: Ollama roda local (zero); OpenRouter usa o preço guardado; o resto é desconhecido.
fn priced(provider: &str, model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    match provider {
        "ollama" => Some(0.0),
        "openrouter" => {
            let (input, output) = *OPENROUTER_PRICES.lock().ok()?.get(model)?;
            Some(input * input_tokens as f64 + output * output_tokens as f64)
        }
        _ => None,
    }
}

/// Custo em USD de um turno. Modelo do OpenRouter ainda sem preço conhecido conta como
/// desconhecido NESTE turno e dispara (no máximo a cada 10 min) a busca do catálogo em segundo
/// plano, para os próximos saírem com valor — nunca atrasa a resposta esperando a rede.
pub fn cost_for(app: &AppHandle, provider: &str, model: &str, usage: &rig_core::completion::Usage) -> Option<f64> {
    let cost = priced(provider, model, usage.input_tokens.unwrap_or(0), usage.output_tokens.unwrap_or(0));
    if cost.is_none() && provider == "openrouter" {
        refresh_openrouter_prices(app);
    }
    cost
}

fn refresh_openrouter_prices(app: &AppHandle) {
    let now = now_ms();
    let last = LAST_PRICE_REFRESH_MS.load(Ordering::Relaxed);
    if now - last < PRICE_REFRESH_INTERVAL_MS || LAST_PRICE_REFRESH_MS.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_err() {
        return;
    }
    let Ok(store) = crate::state::config_store(app) else { return };
    tauri::async_runtime::spawn(async move { super::llm::refresh_openrouter_prices(&store).await });
}

pub fn arm(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Embedding remoto (OpenRouter/OpenAI-compatível) que devolveu `usage.prompt_tokens`, surface
/// `"embeddings"`. Sem [`arm`] (testes) não grava nada.
pub(crate) fn record_embedding(provider: &str, model: &str, input_tokens: u64) {
    let Some(app) = APP.get() else { return };
    let cost = priced(provider, model, input_tokens, 0);
    if cost.is_none() && provider == "openrouter" {
        refresh_openrouter_prices(app);
    }
    record_tokens(app, "embeddings", provider, model, (input_tokens, 0, input_tokens), cost);
}

/// `surface` identifica quem gastou o token: `"chat"`, `"vision"`, `"gamer-summary"` e
/// `"embeddings"` (ver `specs/10-observability.md`, "Custo").
pub fn record(app: &AppHandle, surface: &str, provider: &str, model: &str, usage: &rig_core::completion::Usage, cost_usd: Option<f64>) {
    // Contador não informado pelo provedor (`None`) conta zero, como o `u64` antigo da `rig-core` ≤ 0.42.
    let (input, output, total) = (usage.input_tokens.unwrap_or(0), usage.output_tokens.unwrap_or(0), usage.total_tokens.unwrap_or(0));
    record_tokens(app, surface, provider, model, (input, output, total), cost_usd);
}

fn record_tokens(app: &AppHandle, surface: &str, provider: &str, model: &str, (input, output, total): (u64, u64, u64), cost_usd: Option<f64>) {
    let now_ms = now_ms();
    let (app, surface, provider, model) = (app.clone(), surface.to_string(), provider.to_string(), model.to_string());
    // Dois UPSERTs em SQLite no fim de todo turno: fora do worker async, e sem esperar — contabilidade
    // não pode atrasar a resposta que o usuário já está esperando.
    drop(tauri::async_runtime::spawn_blocking(move || {
        let result = super::telemetry::token_usage_db(&app).and_then(|db| {
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            record_with(&conn, input, output, total, cost_usd, now_ms)?;
            record_by_provider_with(&conn, &surface, &provider, &model, input, output, total, cost_usd, now_ms)
        });
        if let Err(error) = result {
            log::warn!("uso de tokens não registrado: {error}");
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::telemetry::create_token_usage_table(&conn).unwrap();
        conn
    }

    #[test]
    fn turns_accumulate_and_unknown_cost_is_counted_not_summed_as_zero() {
        let conn = conn();
        record_with(&conn, 100, 20, 0, Some(0.0), 1_000).unwrap();
        record_with(&conn, 50, 10, 60, None, 2_000).unwrap();
        let stats = super::super::telemetry::token_usage_get_with(&conn).unwrap();
        assert_eq!(stats["totalTokens"], 180);
        assert_eq!(stats["totalPromptTokens"], 150);
        assert_eq!(stats["totalCompletionTokens"], 30);
        assert_eq!(stats["totalInteractions"], 2);
        assert_eq!(stats["unknownCostTurns"], 1);
        assert_eq!(stats["firstUsedAt"], 1_000);
        assert_eq!(stats["lastUsedAt"], 2_000);
    }

    #[test]
    fn a_non_finite_cost_never_poisons_the_total() {
        let conn = conn();
        record_with(&conn, 1, 1, 2, Some(f64::NAN), 1).unwrap();
        assert_eq!(super::super::telemetry::token_usage_get_with(&conn).unwrap()["totalCostUsd"], 0.0);
    }

    #[test]
    fn ollama_is_free_and_openrouter_is_priced_from_the_live_catalog_or_unknown() {
        assert_eq!(priced("ollama", "qwen3.5:4b", 1_000, 1_000), Some(0.0));
        assert_eq!(priced("openrouter", "test/unseen-model", 1_000, 1_000), None, "sem catálogo, desconhecido");
        remember_openrouter_prices(&[
            serde_json::json!({ "id": "test/priced-model", "promptPrice": "0.000001", "completionPrice": "0.000004" }),
            serde_json::json!({ "id": "test/no-price", "promptPrice": null, "completionPrice": "0.1" }),
        ]);
        let cost = priced("openrouter", "test/priced-model", 1_000, 500).unwrap();
        assert!((cost - 0.003).abs() < 1e-12, "{cost}");
        assert_eq!(priced("openrouter", "test/no-price", 1, 1), None);
        assert_eq!(priced("google", "gemini-3.8-flash", 1, 1), None, "Google não publica preço por API");
    }

    fn provider_row(conn: &Connection, surface: &str, provider: &str, model: &str) -> (i64, i64) {
        conn.query_row(
            "SELECT total_tokens, total_interactions FROM token_usage_by_provider \
                WHERE surface = ?1 AND provider = ?2 AND model = ?3",
            params![surface, provider, model],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn breaks_down_usage_by_surface_provider_and_model_without_mixing_them() {
        let conn = conn();
        record_by_provider_with(&conn, "chat", "ollama", "llama3.2", 100, 20, 0, Some(0.0), 1_000).unwrap();
        record_by_provider_with(&conn, "chat", "ollama", "llama3.2", 50, 10, 0, Some(0.0), 2_000).unwrap();
        record_by_provider_with(&conn, "chat", "google", "gemini-2.5-flash", 30, 5, 0, None, 3_000).unwrap();

        assert_eq!(provider_row(&conn, "chat", "ollama", "llama3.2"), (180, 2));
        assert_eq!(provider_row(&conn, "chat", "google", "gemini-2.5-flash"), (35, 1));
    }

    /// `record()` grava as duas visões do mesmo turno — total agregado e quebra por provider — sem
    /// deixar uma delas para trás.
    #[test]
    fn record_with_and_record_by_provider_with_agree_on_the_same_turn() {
        let conn = conn();
        record_with(&conn, 100, 20, 0, Some(0.0), 1_000).unwrap();
        record_by_provider_with(&conn, "chat", "ollama", "llama3.2", 100, 20, 0, Some(0.0), 1_000).unwrap();

        let total = super::super::telemetry::token_usage_get_with(&conn).unwrap();
        assert_eq!(total["totalTokens"], 120);
        assert_eq!(provider_row(&conn, "chat", "ollama", "llama3.2"), (120, 1));
    }
}
