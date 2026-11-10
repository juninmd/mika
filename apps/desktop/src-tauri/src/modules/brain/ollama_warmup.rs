//! Aquecimento do Ollama: o primeiro turno depois do boot (ou depois de o Ollama ter descarregado o
//! modelo) pagava a carga inteira do modelo — segundos — antes do primeiro token. Aqui o modelo é
//! carregado em segundo plano no boot e de novo quando o VAD ouve o usuário começar a falar: a carga
//! corre em paralelo com a fala + transcrição, não depois delas.
//!
//! O pedido é o `POST /api/generate` sem `prompt` ([`super::turn::ollama_warmup_body`]): o Ollama só
//! carrega e responde, sem gerar nada; com o modelo já carregado ele volta na hora e só renova o
//! `keep_alive`. Mesmo `num_ctx` do turno — outro valor faria o turno recarregar o runner.
//!
//! Só quando o provedor do chat é `ollama` (um provedor remoto não tem modelo local para aquecer), e
//! sempre atrás de `assert_network_allowed`, como o turno.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use crate::config::store::ConfigStore;

/// Carga de modelo grande em disco frio passa de um minuto; o pedido é de fundo, não trava ninguém.
const WARMUP_TIMEOUT: Duration = Duration::from_secs(180);
/// Fala contínua acorda a task a cada janela de voz; um aquecimento por minuto já mantém o modelo
/// carregado (`keep_alive` é bem maior que isso).
const MIN_INTERVAL: Duration = Duration::from_secs(60);

fn wake() -> &'static Notify {
    static WAKE: OnceLock<Notify> = OnceLock::new();
    WAKE.get_or_init(Notify::new)
}

/// O usuário começou a falar: pede um aquecimento. Só acorda a task (nunca bloqueia, nunca faz
/// rede daqui) — seguro de chamar a cada janela do VAD.
pub fn nudge() {
    wake().notify_one();
}

/// Boot: aquece já, e de novo a cada [`nudge`] espaçado de [`MIN_INTERVAL`].
pub fn spawn(store: Arc<ConfigStore>) {
    // `tauri::async_runtime::spawn`, não `tokio::spawn`: chamado de `lib.rs::setup`, fora do reator.
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        let mut last: Option<Instant> = None;
        loop {
            if last.map_or(true, |at| at.elapsed() >= MIN_INTERVAL) {
                warm(&store).await;
                last = Some(Instant::now());
            }
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = wake().notified() => {}
            }
        }
    });
}

async fn warm(store: &ConfigStore) {
    if super::llm_provider::resolve_provider_id(store) != "ollama" {
        return;
    }
    let config = super::turn::resolve_ollama_config_for_model(store).await;
    if let Err(reason) = crate::modules::privacy::assert_network_allowed(store, &config.base_url, "brain:warmup:ollama") {
        log::debug!("ollama: aquecimento não saiu ({reason})");
        return;
    }
    let started = Instant::now();
    let mut request = crate::modules::llm::http_client()
        .post(format!("{}/api/generate", config.base_url.trim_end_matches('/')))
        .timeout(WARMUP_TIMEOUT)
        .json(&super::turn::ollama_warmup_body(&config));
    if !config.api_key.is_empty() {
        request = request.bearer_auth(&config.api_key);
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => {
            log::debug!("ollama: modelo {} pronto em {}ms (aquecimento)", config.model, started.elapsed().as_millis());
        }
        Ok(response) => log::debug!("ollama: aquecimento recusado (HTTP {})", response.status().as_u16()),
        Err(error) => log::debug!("ollama: aquecimento sem resposta: {error}"),
    }
}
