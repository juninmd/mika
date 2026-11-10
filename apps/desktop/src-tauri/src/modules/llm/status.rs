//! `llm:getStatus` / `llm:status` — o estado AO VIVO do cérebro: o Ollama responde? Que versão?
//! Que modelos estão instalados (`/api/tags`) e quais estão carregados na VRAM agora (`/api/ps`)?
//! Mais a latência dos últimos turnos ([`super::turn_metrics`]) e o e5 local do embedding.
//!
//! Também é a fonte de `hasLLM` em `mika:get-health`: antes era `true` fixo ("LLM é compilado no
//! binário"), então a tela dizia que havia cérebro com o Ollama desligado e nenhuma chave salva.
//!
//! Empurrar à tela (`llm:status`) pede um `AppHandle`, que um turno de chat não carrega. O primeiro
//! `invoke` de uma janela dona (`mika:get-health` no boot, ou este próprio canal) [`arm`]a o
//! emissor; antes disso não há tela ouvindo, então não há o que perder.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::store::ConfigStore;
use crate::modules::inference::embedder;
use crate::modules::privacy;
use crate::protocol::WindowType;
use crate::shell_actions::is_owner_surface;

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.LlmStatus` (`desktop-ipc-channels.ts`).
pub const STATUS_EVENT: &str = "llm:status";
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// `mika:get-health` é chamado a cada abertura de tela; sondar o Ollama a cada chamada é rede à toa.
const HEALTH_PROBE_MAX_AGE: Duration = Duration::from_secs(15);

static APP: OnceLock<AppHandle> = OnceLock::new();
static LAST_PROBE: Mutex<Option<(Instant, Value)>> = Mutex::new(None);

pub fn arm(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Ollama pronto = respondeu; remoto pronto = tem a credencial/endpoint de que precisa. Não sonda
/// provedor remoto: cada sondagem ali custa cota e sai para a internet.
pub fn provider_ready(provider: &str, has_credential: bool, ollama_reachable: bool) -> bool {
    match provider {
        "ollama" => ollama_reachable,
        _ => has_credential,
    }
}

fn stored(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn has_credential(store: &ConfigStore, provider: &str) -> bool {
    let env = |name: &str| std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
    match provider {
        "google" => !stored(store, super::CFG_GOOGLE_AI).trim().is_empty() || env("GOOGLE_API_KEY"),
        "openrouter" => !stored(store, super::CFG_OPENROUTER).trim().is_empty() || env("OPENROUTER_API_KEY"),
        // Pronto = tem endereço (a chave é opcional: o servidor responde 401 com o motivo, que sobe
        // ao usuário). O Unsloth tem endereço padrão, então só fica "sem credencial" se o apagarem.
        id => super::openai_shaped(id).is_some_and(|shaped| !stored(store, shaped.base_url_path).trim().is_empty()),
    }
}

fn active_provider(store: &ConfigStore) -> String {
    super::normalize_provider(&stored(store, "llm.provider"))
}

async fn get_json(url: &str, api_key: Option<&str>) -> Result<Value, String> {
    let mut request = super::http_client().get(url).timeout(PROBE_TIMEOUT);
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }
    let response = request.send().await.map_err(|_| "Ollama não respondeu".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Ollama respondeu {}", response.status().as_u16()));
    }
    response.json().await.map_err(|_| "Ollama: resposta inválida".to_string())
}

pub fn installed_from_tags(body: &Value) -> Vec<Value> {
    let models = body.get("models").and_then(Value::as_array).cloned().unwrap_or_default();
    models
        .iter()
        .filter_map(|m| {
            let name = m.get("name").and_then(Value::as_str)?;
            let details = m.get("details").cloned().unwrap_or(Value::Null);
            let family = details.get("family").and_then(Value::as_str).unwrap_or("");
            Some(json!({
                "name": name,
                "sizeBytes": m.get("size").and_then(Value::as_u64),
                "family": family,
                "parameterSize": details.get("parameter_size"),
                "quantization": details.get("quantization_level"),
                "embedding": super::looks_like_embedding(name, family),
            }))
        })
        .collect()
}

pub fn loaded_from_ps(body: &Value) -> Vec<Value> {
    let models = body.get("models").and_then(Value::as_array).cloned().unwrap_or_default();
    models
        .iter()
        .filter_map(|m| {
            Some(json!({
                "name": m.get("name").and_then(Value::as_str)?,
                "sizeBytes": m.get("size").and_then(Value::as_u64),
                "sizeVramBytes": m.get("size_vram").and_then(Value::as_u64),
                "expiresAt": m.get("expires_at"),
            }))
        })
        .collect()
}

/// As três leituras do Ollama em paralelo. `/api/tags` decide `reachable`; `version`/`ps` que
/// falharem sozinhos só ficam vazios (servidor antigo sem `/api/ps` continua "no ar").
async fn probe_ollama(store: &ConfigStore) -> Value {
    let endpoint = crate::modules::desktop::resolve_ollama_endpoint(store);
    let base = super::strip_trailing_slash_and_v1(&endpoint.base_url);
    if let Err(error) = privacy::assert_network_allowed(store, &base, "llm:ollama-status") {
        return json!({ "endpoint": base, "reachable": false, "version": null, "installed": [], "loaded": [], "error": error });
    }
    let key = endpoint.api_key.as_deref();
    let (tags_url, version_url, ps_url) = (format!("{base}/api/tags"), format!("{base}/api/version"), format!("{base}/api/ps"));
    let (tags, version, ps) = tokio::join!(get_json(&tags_url, key), get_json(&version_url, key), get_json(&ps_url, key));
    match tags {
        Ok(tags) => json!({
            "endpoint": base,
            "reachable": true,
            "version": version.ok().and_then(|v| v.get("version").cloned()),
            "installed": installed_from_tags(&tags),
            "loaded": ps.map(|ps| loaded_from_ps(&ps)).unwrap_or_default(),
            "error": null,
        }),
        Err(error) => json!({ "endpoint": base, "reachable": false, "version": null, "installed": [], "loaded": [], "error": error }),
    }
}

fn local_embedding(app: &AppHandle) -> Value {
    let data_dir = crate::state::app_data_dir(app).ok();
    let downloaded = data_dir.as_deref().is_some_and(embedder::is_downloaded);
    let size: Option<u64> = data_dir.filter(|_| downloaded).map(|dir| {
        let cache = embedder::cache_dir(&dir);
        embedder::FILES.iter().filter_map(|(file, _, _)| std::fs::metadata(cache.join(file)).ok()).map(|m| m.len()).sum()
    });
    let model = embedder::MODEL_ID.rsplit('/').next().unwrap_or(embedder::MODEL_ID);
    json!({ "model": model, "dimensions": embedder::DIMENSIONS, "downloaded": downloaded, "sizeBytes": size })
}

fn compose(app: &AppHandle, store: &ConfigStore, ollama: Value) -> Value {
    let provider = active_provider(store);
    let reachable = ollama["reachable"].as_bool().unwrap_or(false);
    json!({
        "provider": provider,
        "ready": provider_ready(&provider, has_credential(store, &provider), reachable),
        "ollama": ollama,
        "localEmbedding": local_embedding(app),
        // Download em curso sobrevive a fechar e reabrir a aba: a tela retoma a barra por aqui.
        "pullingModel": super::pull::active_model(),
        "timings": super::turn_metrics::snapshot(),
    })
}

async fn fresh_probe(store: &ConfigStore) -> Value {
    let probe = probe_ollama(store).await;
    if let Ok(mut last) = LAST_PROBE.lock() {
        *last = Some((Instant::now(), probe.clone()));
    }
    probe
}

fn cached_probe(max_age: Duration) -> Option<Value> {
    let last = LAST_PROBE.lock().ok()?;
    last.as_ref().filter(|(at, _)| at.elapsed() <= max_age).map(|(_, v)| v.clone())
}

/// `llm:getStatus`: sempre sonda de novo — é o botão "atualizar" e a abertura da aba.
pub async fn get_status(app: &AppHandle, store: &ConfigStore) -> Value {
    let probe = fresh_probe(store).await;
    let status = compose(app, store, probe);
    emit(app, &status);
    status
}

/// `hasLLM` de `mika:get-health`, com sondagem de até [`HEALTH_PROBE_MAX_AGE`].
pub async fn has_llm(store: &ConfigStore) -> bool {
    let provider = active_provider(store);
    if provider != "ollama" {
        return provider_ready(&provider, has_credential(store, &provider), false);
    }
    let probe = match cached_probe(HEALTH_PROBE_MAX_AGE) {
        Some(probe) => probe,
        None => fresh_probe(store).await,
    };
    probe["reachable"].as_bool().unwrap_or(false)
}

fn emit(app: &AppHandle, status: &Value) {
    for label in app.webview_windows().keys() {
        if is_owner_surface(WindowType::from_label(label)) {
            let _ = app.emit_to(label.as_str(), STATUS_EVENT, status.clone());
        }
    }
}

/// Algo mudou fora de uma chamada do renderer (turno medido, download terminou): empurra o
/// estado com a última sondagem conhecida — sem sair para a rede de novo por causa de um turno.
pub fn notify() {
    let Some(app) = APP.get() else { return };
    let Ok(store) = crate::state::config_store(app) else { return };
    let probe = cached_probe(Duration::MAX).unwrap_or_else(|| json!({ "reachable": false, "installed": [], "loaded": [] }));
    emit(app, &compose(app, &store, probe));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_readiness_is_reachability_and_remote_readiness_is_credential() {
        assert!(provider_ready("ollama", false, true));
        assert!(!provider_ready("ollama", true, false), "chave não liga um Ollama desligado");
        assert!(provider_ready("google", true, false));
        assert!(!provider_ready("openrouter", false, true), "Ollama no ar não dá cérebro a quem escolheu OpenRouter");
    }

    #[test]
    fn unsloth_counts_as_a_credentialed_remote_when_it_has_an_address() {
        let dir = std::env::temp_dir().join(format!("mika-status-unsloth-{}", std::process::id()));
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert!(has_credential(&store, "unsloth"), "endereço padrão http://localhost:8888/v1");
        store.set("apiKeys.unslothBaseUrl", json!("")).unwrap();
        assert!(!has_credential(&store, "unsloth"), "sem endereço não há o que sondar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tags_become_installed_models_with_size_and_embedding_flag() {
        let body = json!({ "models": [
            { "name": "qwen3:8b", "size": 5_200_000_000u64, "details": { "family": "qwen3", "parameter_size": "8.2B", "quantization_level": "Q4_K_M" } },
            { "name": "nomic-embed-text:latest", "size": 274_000_000u64, "details": { "family": "nomic-bert" } },
            { "size": 1 }
        ]});
        let installed = installed_from_tags(&body);
        assert_eq!(installed.len(), 2, "entrada sem nome é descartada");
        assert_eq!(installed[0]["sizeBytes"], 5_200_000_000u64);
        assert_eq!(installed[0]["parameterSize"], "8.2B");
        assert_eq!(installed[0]["embedding"], false);
        assert_eq!(installed[1]["embedding"], true);
    }

    #[test]
    fn ps_becomes_loaded_models_with_vram() {
        let body = json!({ "models": [{ "name": "qwen3:8b", "size": 6_000u64, "size_vram": 5_500u64, "expires_at": "2026-09-28T12:00:00Z" }] });
        let loaded = loaded_from_ps(&body);
        assert_eq!(loaded[0]["sizeVramBytes"], 5_500u64);
        assert_eq!(loaded[0]["expiresAt"], "2026-09-28T12:00:00Z");
        assert!(loaded_from_ps(&json!({})).is_empty());
    }

    /// O `payload` do evento tem de bater com o canal declarado no contrato TS.
    #[test]
    fn status_event_is_declared_in_the_ts_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("'{STATUS_EVENT}'")));
        assert!(contract.contains(&format!("'{}'", super::super::pull::PROGRESS_EVENT)));
    }
}
