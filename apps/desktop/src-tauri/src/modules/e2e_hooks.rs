//! Ganchos de teste E2E — a única forma de o Playwright disparar EFEITOS que a UI sozinha não
//! aciona de fora (evento de barramento sintético, sinal de energia, redimensionar a janela,
//! capturar seus pixels, ler defaults de config, encerrar de forma graciosa). Existe porque o
//! Electron tinha `app.evaluate` (eval no processo principal); Tauri não tem equivalente, e sem
//! isto seis specs ficariam vermelhas por falta de um jeito honesto de injetar o estímulo — não
//! por defeito de produto.
//!
//! **Dupla trava, e as duas fail-closed:**
//! 1. `MIKA_E2E=1` lido do ambiente do PROCESSO uma única vez (`OnceLock`, primeira leitura vale
//!    para a vida do processo — é assim que "só em build de teste" se garante sem depender do
//!    chamador se lembrar de checar). Sem a variável, [`modules`] devolve um `Vec` vazio: o canal
//!    nem é REIVINDICADO pelo `NativeRegistry` (`native/mod.rs`), então em produção ele não existe
//!    — cai no mesmo erro nomeado de qualquer canal desconhecido (`bridge::unowned_channel`), não
//!    num "acesso negado" que confirmaria a existência da rota para quem está sondando.
//! 2. `is_owner_surface` — a MESMA guarda de `update.rs`/`voice/mod.rs`/`desktop.rs` — roda dentro
//!    de `invoke` mesmo com a variável ligada: uma `plugin-view` nunca aciona isto, nem em build
//!    de teste. Redundante com (1) de propósito: (1) tira o canal do ar em produção, (2) garante
//!    que nem em ambiente de teste uma superfície não confiável ganhe o atalho.
//!
//! **Cada operação mapeia para um caminho de produção real, não uma simulação:**
//! - `publish-event` publica em `crate::events::bus()` — o MESMO barramento que
//!   `modules/system_signals/power.rs`, `brain::awareness` etc. usam de verdade.
//! - `power-signal` chama `bus.publish(EventKey::PowerResume/PowerBattery/PowerAc, ...)` — a MESMA
//!   chamada que `system_signals::power::spawn_power_signal_listener` faz ao detectar a
//!   transição real; só a DETECÇÃO é substituída (não dá para forçar o SO a suspender via IPC).
//! - `window-set-size`/`window-get-bounds` chamam `WebviewWindow::set_size`/`outer_position`+
//!   `inner_size` direto — a mesma API que `shell_actions` usa para mover/redimensionar de
//!   verdade.
//! - `capture-main-window` usa `xcap::Window`, a MESMA biblioteca que `capture.rs` usa para a
//!   visão da Mika — ver a ressalva de alfa abaixo.
//! - `config-defaults` lê `ConfigStore::schema().defaults()` — o mesmo registro de schema que
//!   populariza o store real na primeira abertura.
//! - `quit` chama `crate::tray::mark_quitting` + `app.exit(0)` — a MESMA sequência do item
//!   "Sair" da bandeja (`tray.rs::ID_QUIT`), que dispara `RunEvent::ExitRequested` em `lib.rs` e
//!   roda os shutdown hooks reais (drop dos `RunningService` de MCP, persistência do store).
//!
//! **`capture-main-window` não consegue capturar a `main`, e não é caveat de alfa — é exclusão por
//! design.** `xcap::Window::all()` filtra fora QUALQUER janela do PRÓPRIO processo chamador
//! (`lp_dw_process_id == GetCurrentProcessId()` → excluída, `windows/impl_window.rs` do crate
//! 0.9.8 — herdado do guard do WebRTC contra um seletor de captura se auto-capturar). Como este
//! gancho roda DENTRO do próprio `mika-desktop.exe`, a lista nunca contém a janela `main`: não é
//! um problema de GDI não ver o alfa da composição DWM (isso também seria verdade, mas nunca chega
//! a importar — a busca por pid já devolve zero janelas antes de qualquer captura). `capture.rs`
//! nunca bateu nisso porque a visão da Mika sempre captura a TELA ou OUTRAS janelas, nunca a
//! própria. `avatar-background-modes.spec.ts` (única chamadora) parou de usar este op — usa
//! `capturePagePixels` (`e2e/fixtures/tauri-app.ts`), que lê a página real via
//! `page.screenshot({ omitBackground: true })` (o compositor do Chromium/WebView2, não o SO) e
//! decodifica o PNG dentro da própria página (canvas 2D via `data:` URL). O op continua registrado
//! aqui — inofensivo, só nunca vai achar a janela — para não apagar trabalho de outra frente sem
//! consenso; qualquer spec futura que precise capturar OUTRA janela (não a própria) pode retomá-lo.

use std::sync::OnceLock;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use xcap::Window as XcapWindow;

use crate::events::{bus, EventKey};
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

const CHANNELS: &[&str] = &["mika:e2e-hook"];

/// Só a primeira leitura conta — mesmo motivo de `crate::events::bus::bus()` ser `OnceLock`: o
/// valor vale para a vida do processo, e o binário do Playwright só decide isso na hora que
/// invoca o `.exe` (`MIKA_E2E=1` fixo no `env` do processo, nunca mutado depois do boot).
fn e2e_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| e2e_env_says_enabled(std::env::var("MIKA_E2E").ok().as_deref()))
}

/// Puro, para o teste não precisar mexer no ambiente real do processo de `cargo test` (que é
/// compartilhado por todos os testes rodando em paralelo — mesma armadilha documentada em
/// `events/bus.rs`).
fn e2e_env_says_enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

fn key_from_str(name: &str) -> Option<EventKey> {
    EventKey::ALL.iter().copied().find(|key| key.as_str() == name)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

async fn handle_publish_event(payload: &Value) -> Result<Value, String> {
    let key_name = payload["eventKey"].as_str().ok_or("eventKey ausente")?;
    let key = key_from_str(key_name).ok_or_else(|| format!("EventKey desconhecida: {key_name}"))?;
    bus().publish(key, payload["payload"].clone());
    Ok(json!({ "published": key_name }))
}

fn handle_power_signal(payload: &Value) -> Result<Value, String> {
    let kind = payload["kind"].as_str().ok_or("kind ausente (resume|battery|ac)")?;
    let key = match kind {
        "resume" => EventKey::PowerResume,
        "battery" => EventKey::PowerBattery,
        // A borda simétrica de "battery": religou na tomada. Distinta de "resume" (o SO saiu de
        // suspensão) — `hud_bridge.rs` só encaminha `PowerAc`/`PowerBattery` ao renderer, então
        // uma spec que queira PROVAR o postFx voltando (não só desligando) precisa desta borda, e
        // não da de resume.
        "ac" => EventKey::PowerAc,
        other => return Err(format!("kind desconhecido: {other}")),
    };
    bus().publish(key, json!({ "at": now_ms() }));
    Ok(json!({ "published": kind }))
}

fn handle_window_set_size(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let width = payload["width"].as_f64().ok_or("width ausente")?;
    let height = payload["height"].as_f64().ok_or("height ausente")?;
    let window = app
        .get_webview_window(crate::windows::MAIN)
        .ok_or("janela main não existe")?;
    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn handle_window_get_bounds(app: &AppHandle) -> Result<Value, String> {
    let window = app
        .get_webview_window(crate::windows::MAIN)
        .ok_or("janela main não existe")?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    Ok(json!({ "x": position.x, "y": position.y, "width": size.width, "height": size.height }))
}

/// Ver a ressalva de alfa no cabeçalho do arquivo — o retorno é o melhor esforço honesto, não uma
/// promessa de canal alfa real.
///
/// `payload.samplePoints` (opcional): lista de `[x, y]` em pixel — devolvidos decodificados como
/// `[r, g, b, a]` em `samples`, para a spec comparar cantos/bordas sem precisar de um decodificador
/// de PNG no lado Node (nenhuma dependência de imagem existe em `package.json` hoje, e adicionar
/// uma só para isto seria desproporcional ao gancho de teste).
fn handle_capture_main_window(payload: &Value) -> Result<Value, String> {
    use base64::Engine as _;

    let pid = std::process::id();
    let windows = XcapWindow::all().map_err(|e| e.to_string())?;
    let target = windows
        .into_iter()
        .find(|w| w.pid().map(|p| p == pid).unwrap_or(false) && !w.is_minimized().unwrap_or(true))
        .ok_or("nenhuma janela top-level deste processo encontrada para captura (xcap/GDI não enxerga o compositor do WebView2 por trás de um HWND filho)")?;

    let image = target.capture_image().map_err(|e| e.to_string())?;
    if image.width() == 0 || image.height() == 0 {
        return Err("captura devolveu 0 pixels".into());
    }
    let (width, height) = (image.width(), image.height());

    let samples: Vec<Value> = payload["samplePoints"]
        .as_array()
        .map(|points| {
            points
                .iter()
                .map(|point| {
                    let x = point[0].as_u64().unwrap_or(0).min((width.saturating_sub(1)) as u64) as u32;
                    let y = point[1].as_u64().unwrap_or(0).min((height.saturating_sub(1)) as u64) as u32;
                    let pixel = image.get_pixel(x, y).0;
                    json!([pixel[0], pixel[1], pixel[2], pixel[3]])
                })
                .collect()
        })
        .unwrap_or_default();

    let mut buffer = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "base64": base64::engine::general_purpose::STANDARD.encode(&buffer),
        "width": width,
        "height": height,
        "samples": samples,
        "alphaCaveat": "GDI BitBlt/PrintWindow não captura o alfa da composição DWM — ver cabeçalho de e2e_hooks.rs",
    }))
}

/// Grava o snapshot de janela ativa que o gate de captura consulta
/// (`vision::capture_gate::is_foreground_blocked`). No E2E a janela em foco é a PRÓPRIA Mika, que a
/// captura recusa de propósito (ela nunca analisa a própria janela); sem este gancho a jornada de
/// visão só passava quando a sonda ainda não tinha lido nenhuma janela. Só a origem do snapshot é
/// sintética — o gate (blocklist, consentimento, janela própria) roda inteiro, de verdade.
fn handle_set_active_window(payload: &Value) -> Result<Value, String> {
    let text = |key: &str| payload[key].as_str().map(str::to_owned);
    let snapshot = crate::modules::system_signals::active_window::ActiveWindowSnapshot {
        title: text("title"),
        owner: text("owner"),
    };
    crate::modules::system_signals::active_window::remember(snapshot);
    Ok(Value::Null)
}

fn handle_config_defaults(app: &AppHandle) -> Result<Value, String> {
    let store = crate::state::config_store(app)?;
    Ok(store.schema().defaults())
}

/// Grava uma meta pela MESMA `add_goal` que `brain::reflection::run_reflection` chama de verdade
/// após uma reflexão bem avaliada — só a origem é sintética (id/descrição vêm do teste), a
/// escrita cai no mesmo `MemoryStore`/SQLite de produção. Ver `shutdown-persistence.spec.ts`.
fn handle_add_brain_goal(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_str().ok_or("id ausente")?;
    let description = payload["description"].as_str().ok_or("description ausente")?;
    let priority = payload["priority"].as_f64().unwrap_or(0.5);
    let mem = crate::modules::memory::memory_store(app)?;
    crate::modules::brain::reflection::goals::add_goal(mem, id, description, priority, now_ms() as i64)?;
    Ok(Value::Null)
}

/// Alimenta o pipeline REAL de `vision::watcher` (parse do `GAME_SIGNAL:` → auto-upsert de pessoas
/// no `person_catalog` → guarda de `newFacts` na memória episódica → publica
/// `EventKey::ScreenObservation`/`GameMoment`) com uma resposta de modelo fabricada, sem captura de
/// tela nem provedor LLM real — `payload.raw` é o texto cru (comentário + bloco `GAME_SIGNAL:`,
/// MESMO formato que `vision::watcher::DEFAULT_TICK_PROMPT` pede). Existe porque o auto-upsert vive
/// dentro do tick do watcher, não no barramento: publicar `vision:screen-observation` direto via
/// `publish-event` (acima) NUNCA aciona o auto-upsert — só este gancho passa pelo parse de verdade.
/// Devolve o payload publicado, para a spec conferir sem precisar assinar o barramento.
fn handle_vision_ingest(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let raw = payload["raw"].as_str().ok_or("raw ausente")?;
    let store = crate::state::config_store(app)?;
    Ok(crate::modules::vision::ingest_analysis_for_e2e(app, &store, bus(), raw))
}

/// Lê de volta pelo mesmo `MemoryStore` — prova que a escrita chegou ao disco sem depender de um
/// segundo processo reabrindo o arquivo (o `MemoryStore` do processo é o único abridor do arquivo
/// SQLite; reabrir aqui seria o mesmo handle, não uma segunda conexão).
fn handle_list_brain_goals(app: &AppHandle) -> Result<Value, String> {
    let mem = crate::modules::memory::memory_store(app)?;
    let goals = crate::modules::brain::reflection::goals::get_active_goals(mem)?;
    serde_json::to_value(goals).map_err(|e| e.to_string())
}

fn handle_quit(app: &AppHandle) -> Result<Value, String> {
    // Mesma sequência do clique em "Sair" na bandeja (`tray.rs::ID_QUIT`): marca `quitting` e
    // pede a saída, que dispara `RunEvent::ExitRequested` em `lib.rs` e roda os shutdown hooks
    // reais (MCP `disconnect_all`, persistência do store) antes do processo morrer de verdade.
    crate::tray::mark_quitting(app);
    app.exit(0);
    Ok(Value::Null)
}

struct E2eHooksModule;

#[async_trait]
impl NativeModule for E2eHooksModule {
    fn name(&self) -> &'static str {
        "e2e_hooks"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        // Redundante com `modules()` só devolver este módulo quando `MIKA_E2E=1` — mas barato o
        // bastante para não confiar só na composição do registro, e é isto que o teste
        // `rejects_without_the_env_var_even_if_constructed_directly` prova sem contornar o
        // registro de verdade.
        if !e2e_enabled() {
            return Err("ganchos de e2e desligados (MIKA_E2E != 1)".into());
        }
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }
        if channel != "mika:e2e-hook" {
            return Err(format!("canal '{channel}' declarado em `e2e_hooks` mas não tratado"));
        }

        let op = payload["op"].as_str().ok_or("op ausente")?;
        match op {
            "publish-event" => handle_publish_event(&payload).await,
            "power-signal" => handle_power_signal(&payload),
            "window-set-size" => handle_window_set_size(app, &payload),
            "window-get-bounds" => handle_window_get_bounds(app),
            "set-active-window" => handle_set_active_window(&payload),
            "capture-main-window" => handle_capture_main_window(&payload),
            "config-defaults" => handle_config_defaults(app),
            "add-brain-goal" => handle_add_brain_goal(app, &payload),
            "list-brain-goals" => handle_list_brain_goals(app),
            "vision-ingest" => handle_vision_ingest(app, &payload),
            "trigger-tool-approval" => {
                crate::modules::tools::trigger_e2e_approval_request(app)?;
                Ok(Value::Null)
            }
            "quit" => handle_quit(app),
            other => Err(format!("op desconhecida: {other}")),
        }
    }
}

/// Só reivindica o canal quando `MIKA_E2E=1` — sem a variável, o canal simplesmente NÃO EXISTE
/// no `NativeRegistry` (ver o cabeçalho do arquivo). Isto é o que garante "inalcançável em
/// produção" de verdade, e não só "responde com erro".
pub fn modules() -> Vec<Box<dyn NativeModule>> {
    if e2e_enabled() {
        vec![Box::new(E2eHooksModule)]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    /// Sem a variável de ambiente (ou com qualquer valor que não seja o literal `"1"`), o
    /// processo nunca reivindica o canal — a mesma pergunta que `modules()` faz de verdade,
    /// testada pura para não depender do `std::env` compartilhado entre testes paralelos.
    #[test]
    fn rejects_without_the_env_var() {
        assert!(!e2e_env_says_enabled(None));
        assert!(!e2e_env_says_enabled(Some("0")));
        assert!(!e2e_env_says_enabled(Some("true")));
        assert!(!e2e_env_says_enabled(Some("")));
    }

    #[test]
    fn accepts_only_the_exact_literal_one() {
        assert!(e2e_env_says_enabled(Some("1")));
    }

    /// A mesma política de superfície que `update.rs`/`voice/mod.rs`/`desktop.rs` já provam para
    /// as próprias rotas: `main`/`settings`/`wizard` passam, `plugin-view`/`unknown` não — e é
    /// exatamente essa função que `invoke` chama antes de rodar qualquer operação.
    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(is_owner_surface(WindowType::Main));
        assert!(is_owner_surface(WindowType::Settings));
        assert!(is_owner_surface(WindowType::Wizard));
        assert!(!is_owner_surface(WindowType::PluginView));
        assert!(!is_owner_surface(WindowType::Unknown));
    }

    #[test]
    fn channels_match_declared_contract() {
        assert_eq!(CHANNELS, &["mika:e2e-hook"]);
    }

    #[test]
    fn key_from_str_round_trips_every_event_key() {
        for key in EventKey::ALL {
            assert_eq!(key_from_str(key.as_str()), Some(*key));
        }
        assert_eq!(key_from_str("not-a-real-key"), None);
    }
}
