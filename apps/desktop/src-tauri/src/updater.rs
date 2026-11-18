//! Atualização automática via `tauri-plugin-updater`, no lugar do `electron-updater`.
//!
//! A troca não foi preferência: `electron-updater` EXIGE o Electron em runtime e matava o
//! sidecar no boot, antes de qualquer rota subir (ver `shared/tauri-main/auto-updater.ts`).
//!
//! O shim TS preserva a superfície que `update.service.ts` já usava, e ela separa `check` de
//! `download` de `install` em três chamadas. O `Update` do plugin é o objeto que carrega a URL
//! e a assinatura verificada entre elas, então ele fica guardado aqui entre uma chamada e a
//! seguinte — refazer o `check()` a cada passo baixaria de um manifesto possivelmente já
//! diferente do que o usuário aceitou.

use std::sync::Mutex;

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

/// O `Update` do último `check()` e os bytes do último `download()`, aguardando `install()`.
/// Nunca acumula: cada `check()` substitui o anterior e descarta bytes que ninguém instalou.
static PENDING: Mutex<Option<(Update, Option<Vec<u8>>)>> = Mutex::new(None);

/// `updateConfigured` — no Tauri a configuração de update são os `endpoints` do
/// `tauri.conf.json`; sem eles o `build()` do plugin falha com `EmptyEndpoints`. É exatamente
/// a pergunta que `update.service.ts` faz antes de oferecer o controle ao usuário.
pub fn is_configured(app: &AppHandle) -> Value {
    Value::Bool(!cfg!(feature = "steam") && build(app).is_ok())
}

/// O `Updater` do `tauri.conf.json`. No binário E2E, com `MIKA_E2E=1` E `MIKA_E2E_UPDATE_ENDPOINT`,
/// o endpoint é o `latest.json` local do spec — o distribuído nem compila este desvio.
fn build(app: &AppHandle) -> tauri_plugin_updater::Result<Updater> {
    #[cfg(feature = "e2e")]
    if let Some(endpoint) = e2e_endpoint() {
        return app.updater_builder().endpoints(vec![endpoint])?.build();
    }
    app.updater()
}

#[cfg(feature = "e2e")]
fn e2e_endpoint() -> Option<url::Url> {
    if std::env::var("MIKA_E2E").as_deref() != Ok("1") {
        return None;
    }
    std::env::var("MIKA_E2E_UPDATE_ENDPOINT").ok()?.parse().ok()
}

/// `updateCheck` — consulta o endpoint. Assíncrono de verdade (rede), por isso vive no caminho
/// deferido: bloquear aqui travaria a task que bombeia o stdout do sidecar.
pub async fn check(app: AppHandle) -> Result<Value, String> {
    if cfg!(feature = "steam") {
        return Err("atualização gerenciada pelo cliente Steam".into());
    }
    let updater = build(&app).map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;

    let Some(update) = found else {
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Ok(serde_json::json!({ "available": false }));
    };

    let version = update.version.clone();
    // `notes` do `latest.json`: o changelog que o CI monta dos commits convencionais (release.yml).
    let notes = update.body.clone().filter(|body| !body.trim().is_empty());
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, None));
    crate::shell_actions::notify(&app, "updater://available", serde_json::json!({ "version": version }));

    Ok(serde_json::json!({ "available": true, "version": version, "notes": notes }))
}

/// `updateDownload` — baixa e VERIFICA a assinatura (o plugin recusa pacote não assinado pela
/// chave do `tauri.conf.json`), mas não instala: quem instala é `updateInstall`, como no
/// contrato do `electron-updater` que `update.service.ts` já seguia.
pub async fn download(app: AppHandle) -> Result<Value, String> {
    let update = {
        let guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        let Some((update, _)) = guard.as_ref() else {
            return Err("nenhuma atualização pendente — chame updateCheck antes".into());
        };
        update.clone()
    };

    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let percent = total
                    .filter(|t| *t > 0)
                    .map(|t| (downloaded as f64 / t as f64) * 100.0)
                    .unwrap_or(0.0);
                crate::shell_actions::notify(
                    &progress_app,
                    "updater://progress",
                    serde_json::json!({ "percent": percent, "transferred": downloaded, "total": total }),
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    let version = update.version.clone();
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, Some(bytes)));
    crate::shell_actions::notify(&app, "updater://downloaded", serde_json::json!({ "version": version }));

    Ok(Value::Null)
}

/// `updateInstall` — roda o instalador e encerra o app. Chega sem `id` (o shim usa
/// `transport.shell()`, sem esperar resposta), então o erro só pode ir para o canal de erro.
pub async fn install(app: &AppHandle) -> Result<Value, String> {
    let taken = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some((update, Some(bytes))) = taken else {
        let message = "nenhum pacote baixado — chame updateDownload antes";
        crate::shell_actions::notify(app, "updater://error", serde_json::json!({ "message": message }));
        return Err(message.into());
    };

    backup_data_before_install(app, &update.version);
    #[cfg(windows)]
    release_children_before_install().await;

    if let Err(error) = update.install(bytes) {
        let message = error.to_string();
        crate::shell_actions::notify(app, "updater://error", serde_json::json!({ "message": message }));
        return Err(message);
    }

    // No Windows o plugin roda o instalador e encerra o processo ali mesmo (o NSIS reabre o app).
    // No Linux e no macOS `install` só troca o pacote em disco e VOLTA: sem reiniciar, o app seguia
    // na versão antiga, e o próximo check acharia a mesma atualização de novo.
    #[cfg(not(windows))]
    app.request_restart();

    Ok(Value::Null)
}

/// No Windows o plugin chama `std::process::exit` logo após lançar o instalador, pulando o
/// `ExitRequested` de `lib.rs`: loops de fundo e servidores MCP (processos filhos) seguiam vivos
/// enquanto o NSIS tentava matar a Mika ("Failed to kill Mika"). Mesmas etapas do "Sair".
#[cfg(windows)]
async fn release_children_before_install() {
    crate::shutdown::trigger();
    crate::shutdown::within("desconectar MCP", crate::shutdown::STEP_TIMEOUT, crate::modules::mcp::disconnect_all()).await;
    let _ = tauri::async_runtime::spawn_blocking(crate::modules::system::log_files::flush).await;
}

/// Cópia de segurança dos dados antes do instalador (ver `modules/update_backup.rs`). Best-effort:
/// disco cheio no backup não pode impedir o usuário de receber o update, então a falha só é logada.
fn backup_data_before_install(app: &AppHandle, version: &str) {
    let result = crate::state::app_data_dir(app)
        .map_err(|e| e.to_string())
        .and_then(|dir| crate::modules::update_backup::backup_before_update(&dir, version));
    match result {
        Ok(path) => log::info!("backup pré-update em {}", path.display()),
        Err(error) => log::warn!("backup pré-update falhou, seguindo com o update: {error}"),
    }
}
