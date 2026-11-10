//! Módulo `update` em Rust — port de `ipc/update.ts` + `update.service.ts`.
//!
//! A mecânica de atualização (check/download/install via `tauri-plugin-updater`) já vive em
//! `updater.rs`; este módulo só adapta os 4 canais para a MESMA forma de resposta que o
//! renderer já consome (`{ success, updateAvailable, version?, error? }` etc.) e guarda o
//! estado que `updateStatus` expõe — `updater.rs` não tem getter de status, só eventos.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::modules::system::governor;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;
use crate::updater;

pub mod auto;

/// `app.autoUpdate` — ver [`auto::Mode`].
const AUTO_UPDATE_CONFIG_PATH: &str = "app.autoUpdate";
/// De quanto em quanto o laço de fundo reavalia (o estado de energia muda em minutos, não segundos).
const AUTO_TICK: Duration = Duration::from_secs(60);
/// Intervalo entre checagens automáticas; o check do boot (renderer) conta para ele.
const AUTO_CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Espera depois de um download que falhou, antes de tentar de novo.
const AUTO_DOWNLOAD_RETRY: Duration = Duration::from_secs(30 * 60);

const CHANNELS: &[&str] = &[
    "mika:update-check",
    "mika:update-download",
    "mika:update-quit-and-install",
    "mika:update-status",
];

/// Espelha `UpdateStatus` de `update.service.ts`. `progress` só assume `Some(100.0)`: o
/// `download()` de `updater.rs` é bloqueante até o fim, então não há progresso parcial para
/// guardar aqui — o parcial já viaja pelo evento `updater://progress` que `updater.rs` dispara.
#[derive(Default)]
struct UpdateStatus {
    available: bool,
    version: Option<String>,
    progress: Option<f64>,
}

impl UpdateStatus {
    fn to_value(&self) -> Value {
        let mut value = json!({ "available": self.available });
        if let Some(version) = &self.version {
            value["version"] = json!(version);
        }
        if let Some(progress) = self.progress {
            value["progress"] = json!(progress);
        }
        value
    }
}

static STATUS: Mutex<UpdateStatus> = Mutex::new(UpdateStatus {
    available: false,
    version: None,
    progress: None,
});

/// Momento do último check, manual (renderer) ou automático — os dois dividem o mesmo intervalo.
static LAST_CHECK: Mutex<Option<Instant>> = Mutex::new(None);

fn phase() -> auto::Phase {
    let status = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    if status.progress == Some(100.0) {
        auto::Phase::Downloaded
    } else if status.available {
        auto::Phase::Available
    } else {
        auto::Phase::UpToDate
    }
}

async fn handle_check(app: &AppHandle) -> Value {
    // Sem `endpoints` no tauri.conf.json o plugin falha com `EmptyEndpoints`, e o usuário leria
    // isso como "a checagem quebrou" em vez de "esta build não recebe update". A pergunta que
    // `update.service.ts` fazia antes de oferecer o controle continua sendo feita — só que aqui,
    // onde ela muda a mensagem, em vez de num canal que ninguém mais chama.
    if updater::is_configured(app) != Value::Bool(true) {
        return json!({
            "success": false,
            "updateAvailable": false,
            "error": "atualização automática não configurada nesta build (sem endpoints de update)",
        });
    }

    *LAST_CHECK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    match updater::check(app.clone()).await {
        Ok(result) => {
            let available = result.get("available").and_then(Value::as_bool).unwrap_or(false);
            let version = result.get("version").and_then(Value::as_str).map(str::to_owned);
            let notes = result.get("notes").and_then(Value::as_str).map(str::to_owned);

            {
                let mut status = STATUS.lock().unwrap_or_else(|e| e.into_inner());
                status.available = available;
                status.version = version.clone();
                // `updater::check` descarta o pacote do check anterior: "baixado" deixa de ser verdade.
                status.progress = None;
            }

            let mut response = json!({ "success": true, "updateAvailable": available });
            if let Some(version) = version {
                response["version"] = json!(version);
            }
            if let Some(notes) = notes {
                response["notes"] = json!(notes);
            }
            response
        }
        Err(error) => json!({ "success": false, "updateAvailable": false, "error": error }),
    }
}

async fn handle_download(app: &AppHandle) -> Value {
    match updater::download(app.clone()).await {
        Ok(_) => {
            STATUS.lock().unwrap_or_else(|e| e.into_inner()).progress = Some(100.0);
            json!({ "success": true })
        }
        Err(error) => json!({ "success": false, "error": error }),
    }
}

/// Só instala se um `updateCheck` já tiver achado algo — mesma trava de `update.service.ts`
/// (`if (!status.available) return { success: false, error: 'No update available' }`). Sem
/// download prévio, `updater::install` ainda recusa por falta de bytes, então a trava aqui é
/// só o caso comum (evita a chamada e a mensagem genérica do plugin).
fn handle_quit_and_install(app: &AppHandle) -> Value {
    if !STATUS.lock().unwrap_or_else(|e| e.into_inner()).available {
        return json!({ "success": false, "error": "No update available" });
    }

    match updater::install(app) {
        Ok(_) => json!({ "success": true }),
        Err(error) => json!({ "success": false, "error": error }),
    }
}

struct UpdateModule;

#[async_trait]
impl NativeModule for UpdateModule {
    fn name(&self) -> &'static str {
        "update"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        _payload: Value,
    ) -> Result<Value, String> {
        // Mesma guarda de `assertOwnerSurface`: fora do try/catch de cada handler em
        // `ipc/update.ts`, ela também vira aqui um `Err` de verdade, não um `{success:false}`.
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }

        match channel {
            "mika:update-check" => Ok(handle_check(app).await),
            "mika:update-download" => Ok(handle_download(app).await),
            "mika:update-quit-and-install" => Ok(handle_quit_and_install(app)),
            "mika:update-status" => Ok(STATUS.lock().unwrap_or_else(|e| e.into_inner()).to_value()),
            other => Err(format!("canal '{other}' declarado em `update` mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(UpdateModule)]
}

fn auto_mode(store: &ConfigStore) -> auto::Mode {
    auto::Mode::parse(store.get(AUTO_UPDATE_CONFIG_PATH).as_ref().and_then(Value::as_str).unwrap_or_default())
}

/// O instalador falhou (pacote sumiu, disco cheio): volta ao começo para o próximo check refazer o
/// caminho, em vez de tentar instalar o mesmo nada a cada minuto de ócio.
fn forget_pending() {
    let mut status = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    status.available = false;
    status.progress = None;
}

/// Laço de fundo da atualização automática (decisão em [`auto::next_step`]). Não nasce na build
/// Steam nem sem endpoint de update: ali não há o que checar. Um passo por minuto, no máximo.
pub fn spawn_auto(store: Arc<ConfigStore>, app: AppHandle) {
    if updater::is_configured(&app) != Value::Bool(true) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + AUTO_TICK, AUTO_TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut download_failed_at: Option<Instant> = None;
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tick.tick() => {}
            }
            let last_check = *LAST_CHECK.lock().unwrap_or_else(|e| e.into_inner());
            let due = auto::Due {
                check: last_check.is_none_or(|at| at.elapsed() >= AUTO_CHECK_EVERY),
                download: download_failed_at.is_none_or(|at| at.elapsed() >= AUTO_DOWNLOAD_RETRY),
            };
            match auto::next_step(auto_mode(&store), phase(), due, governor::state()) {
                auto::Step::Wait => {}
                auto::Step::Check => {
                    let result = handle_check(&app).await;
                    log::info!("atualização automática: check -> {result}");
                }
                auto::Step::Download => {
                    let result = handle_download(&app).await;
                    let failed = result.get("success").and_then(Value::as_bool) != Some(true);
                    download_failed_at = failed.then(Instant::now);
                    log::info!("atualização automática: download -> {result}");
                }
                auto::Step::Install => {
                    log::info!("atualização automática: PC ocioso, instalando");
                    let result = handle_quit_and_install(&app);
                    if result.get("success").and_then(Value::as_bool) != Some(true) {
                        log::warn!("atualização automática: instalação falhou -> {result}");
                        forget_pending();
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".into(),
            plugin_id: None,
        }
    }

    #[test]
    fn channels_match_the_ts_contract() {
        assert_eq!(
            CHANNELS,
            &[
                "mika:update-check",
                "mika:update-download",
                "mika:update-quit-and-install",
                "mika:update-status",
            ]
        );
    }

    /// `mika:update-status` não bate rede nem plugin — dá para provar a forma da resposta e a
    /// guarda de superfície sem um `AppHandle` de verdade, que exige app Tauri inicializado.
    #[test]
    fn status_shape_omits_absent_fields() {
        let value = UpdateStatus::default().to_value();
        assert_eq!(value, json!({ "available": false }));
        assert!(value.get("version").is_none());
        assert!(value.get("progress").is_none());
    }

    #[test]
    fn status_shape_includes_present_fields() {
        let status = UpdateStatus {
            available: true,
            version: Some("1.2.3".into()),
            progress: Some(100.0),
        };
        assert_eq!(
            status.to_value(),
            json!({ "available": true, "version": "1.2.3", "progress": 100.0 })
        );
    }

    /// `plugin-view` e `unknown` são recusadas — mesma lista de `assertOwnerSurface` em
    /// `surface-policy.ts` (`main`, `settings`, `wizard` passam; o resto não).
    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(is_owner_surface(ctx(WindowType::Main).window_type));
        assert!(is_owner_surface(ctx(WindowType::Settings).window_type));
        assert!(is_owner_surface(ctx(WindowType::Wizard).window_type));
        assert!(!is_owner_surface(ctx(WindowType::PluginView).window_type));
        assert!(!is_owner_surface(ctx(WindowType::Unknown).window_type));
    }
}
