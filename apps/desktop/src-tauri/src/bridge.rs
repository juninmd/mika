//! Os comandos que o renderer chama. Substituem `ipcRenderer.invoke` / `.send`.
//!
//! O renderer manda canal e payload. Ele NÃO manda quem é — o contexto sai do label da
//! janela que fez a chamada, que o Tauri conhece e o webview não escolhe. Era esse o
//! furo do `detectWindowType`: a URL do `webContents` decidia o nível de confiança.

use serde_json::Value;
use tauri::{AppHandle, Manager, Webview};

use crate::protocol::{InvokeContext, WindowType};
use crate::shell_actions;
use crate::state::ShellState;

/// Deriva o contexto do label da janela chamadora. Label desconhecido vira `Unknown`,
/// que não passa na guarda de superfície.
fn context_from(webview: &Webview) -> InvokeContext {
    context_from_label(webview.window().label())
}

pub(crate) fn context_from_label(label: &str) -> InvokeContext {
    let label = label.to_string();
    let window_type = WindowType::from_label(&label);

    // `plugin-view::<pluginId>::<viewId>` — o id do plugin vem do label, não da query string.
    let plugin_id = if window_type == WindowType::PluginView {
        crate::windows::plugin_id_from_label(&label)
    } else {
        None
    };

    InvokeContext {
        window_type,
        window_label: label,
        plugin_id,
    }
}

/// Canais resolvidos AQUI, no Rust, porque é o Rust que é dono da janela.
///
/// Os nomes vêm literalmente de `DESKTOP_SHELL_CHANNELS` em
/// packages/sdk/src/contracts/desktop-ipc-channels.ts — não têm prefixo `shell:`. O teste
/// `shell_channels_match_typescript_contract` lê aquele arquivo e falha se divergirem: um
/// nome errado aqui não dá erro, só faz a ação cair no sidecar e nunca acontecer.
///
/// `auto-launch:*` NÃO entra: é registro/arquivo de sistema, que o sidecar Node resolve.
pub const SHELL_CHANNELS: [&str; 12] = [
    "open-settings",
    "window-close",
    "window-toggle-always-on-top",
    "window-toggle-fullscreen",
    "window-get-fullscreen",
    "window-ignore-mouse",
    "window-move",
    "window-start-drag",
    "plugin-view:open",
    "open-external",
    "reveal-path",
    "app-quit",
];

fn shell_channel(channel: &str) -> bool {
    SHELL_CHANNELS.contains(&channel)
}

fn handle_shell(app: &AppHandle, caller: &str, channel: &str, payload: &Value) -> Result<Value, String> {
    match channel {
        "open-settings" => {
            crate::windows::create_settings(app)
                .map(|w| {
                    let _ = w.show();
                    let _ = w.set_focus();
                })
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "window-close" => shell_actions::close_window(app, caller).map(|_| Value::Null),
        "window-toggle-always-on-top" => {
            shell_actions::toggle_always_on_top(app).map(Value::Bool)
        }
        "window-toggle-fullscreen" => crate::fullscreen::toggle(app).map(Value::Bool),
        "window-get-fullscreen" => crate::fullscreen::is_fullscreen(app).map(Value::Bool),
        "window-ignore-mouse" => {
            let ignore = payload.as_bool().ok_or("payload deve ser booleano")?;
            shell_actions::set_ignore_mouse(app, ignore).map(|_| Value::Null)
        }
        "window-move" => {
            let delta_x = payload["deltaX"].as_f64().ok_or("deltaX ausente")?;
            let delta_y = payload["deltaY"].as_f64().ok_or("deltaY ausente")?;
            shell_actions::move_main_window(app, delta_x, delta_y).map(|_| Value::Null)
        }
        "window-start-drag" => shell_actions::start_main_drag(app).map(|_| Value::Null),
        // Propaga o erro: `openPluginView` recusa pluginId/viewId vazio, e engolir isso
        // devolveria "abri" para o renderer com nenhuma janela na tela.
        "plugin-view:open" => shell_actions::handle(app, "openPluginView", payload.clone()),
        // Allowlist de hosts / nome fechado de pasta: a validação mora em `shell_actions`.
        "open-external" => shell_actions::handle(app, "openExternal", payload.clone()),
        "reveal-path" => shell_actions::handle(app, "revealPath", payload.clone()),
        "app-quit" => {
            shell_actions::quit_app(app);
            Ok(Value::Null)
        }
        other => Err(format!("canal de shell desconhecido: {other}")),
    }
}

#[tauri::command]
pub async fn mika_invoke(
    webview: Webview,
    app: AppHandle,
    channel: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    dispatch(&app, &context_from(&webview), &channel, payload).await
}

/// O corpo de `mika_invoke`/`mika_send`, separado do `Webview` para o host HTTP do E2E fora do
/// Windows (`e2e_host.rs`) chamar EXATAMENTE o mesmo caminho de produção, com o contexto derivado
/// do label da janela.
pub(crate) async fn dispatch(
    app: &AppHandle,
    ctx: &InvokeContext,
    channel: &str,
    payload: Option<Value>,
) -> Result<Value, String> {
    if shell_channel(channel) {
        if !shell_actions::is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }
        return handle_shell(app, &ctx.window_label, channel, payload.as_ref().unwrap_or(&Value::Null));
    }

    let state = app.state::<ShellState>();

    if state.native.owns(channel) {
        let payload = payload.unwrap_or(Value::Null);
        return state.native.invoke(app, ctx, channel, payload).await;
    }

    Err(unowned_channel(channel))
}

/// O que sobrou no lugar do fallback para o sidecar Node, apagado em 2026-09-22 com a catraca em
/// 43/43.
///
/// Vira ERRO NOMEADO, nunca silêncio: enquanto havia sidecar, canal sem dono nativo simplesmente
/// descia para o Node e funcionava — foi assim que um prefixo errado em `SHELL_CHANNELS` deixou
/// mover/fechar/fixar janela mortos sem uma linha de log (ver `shell_channels_match_typescript_contract`
/// logo abaixo). Sem o fallback, o mesmo engano some da tela do mesmo jeito se este erro não
/// nomear o canal. A mensagem é para quem adicionar canal novo no contrato e esquecer de dar dono
/// a ele em `native/mod.rs`.
fn unowned_channel(channel: &str) -> String {
    format!("canal '{channel}' não tem dono nativo — o sidecar Node não existe mais, então não há para onde cair")
}

#[tauri::command]
pub async fn mika_send(
    webview: Webview,
    app: AppHandle,
    channel: String,
    payload: Option<Value>,
) -> Result<(), String> {
    // `send` é fire-and-forget para o renderer, mas o erro do módulo nativo não é descartado: ele
    // sobe como rejeição do comando.
    dispatch(&app, &context_from(&webview), &channel, payload).await.map(|_| ())
}

/// Quadro do VAD (`voice:vad-audio`, até ~100 Hz) como corpo binário Float32 LE — sem o ida e volta
/// por JSON de `mika_send`. `amplitude`/`playing` vão nos headers (ver `tauri-renderer-runtime.ts`).
/// O caminho JSON de `mika_send` continua aceito para `number[]`.
#[tauri::command]
pub async fn mika_send_pcm(webview: Webview, app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let ctx = context_from(&webview);
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("mika_send_pcm espera corpo binário".to_string());
    };
    let (amplitude, playing) = parse_vad_headers(request.headers())?;
    crate::modules::voice::vad_audio::handle_binary(&app, &ctx, bytes, amplitude, playing).await
}

fn parse_vad_headers(headers: &tauri::http::HeaderMap) -> Result<(f32, Option<bool>), String> {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let amplitude = text("x-mika-amplitude")
        .and_then(|v| v.parse::<f32>().ok())
        .ok_or("header x-mika-amplitude ausente ou não numérico")?;
    Ok((amplitude, text("x-mika-playing").map(|v| v == "1")))
}

#[cfg(test)]
mod tests {
    use super::{parse_vad_headers, SHELL_CHANNELS};

    #[test]
    fn vad_headers_carry_amplitude_and_optional_playing() {
        let mut headers = tauri::http::HeaderMap::new();
        assert!(parse_vad_headers(&headers).is_err(), "sem amplitude é violação de contrato");
        headers.insert("x-mika-amplitude", "0.25".parse().unwrap());
        assert_eq!(parse_vad_headers(&headers).unwrap(), (0.25, None));
        headers.insert("x-mika-playing", "0".parse().unwrap());
        assert_eq!(parse_vad_headers(&headers).unwrap(), (0.25, Some(false)));
        headers.insert("x-mika-playing", "1".parse().unwrap());
        assert_eq!(parse_vad_headers(&headers).unwrap().1, Some(true));
    }

    /// Existe porque já divergiu: os nomes daqui foram escritos com um prefixo `shell:` que o
    /// contrato TypeScript não tem. Nada falhou — as ações apenas desciam para o sidecar e
    /// nunca aconteciam. Mover, fechar e fixar a janela ficariam mortos, em silêncio.
    #[test]
    fn shell_channels_match_typescript_contract() {
        let contract = include_str!(
            "../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts"
        );

        for channel in SHELL_CHANNELS {
            let literal = format!("'{channel}'");
            assert!(
                contract.contains(&literal),
                "canal {literal} não existe em desktop-ipc-channels.ts — o renderer nunca vai mandá-lo, \
                 e a ação de janela correspondente fica morta sem erro"
            );
        }
    }
}
