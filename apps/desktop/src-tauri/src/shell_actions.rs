//! Operações de janela. Ficam no Rust porque é o Rust que é dono da janela; desde que o sidecar
//! Node foi apagado (2026-09-22) não há mais nenhum outro processo a quem delegá-las.
//! A guarda de superfície continua valendo, aplicada aqui na fronteira.
//!
//! **Lacuna nomeada — diálogos nativos.** `pickFile`/`pickDirectory`/`saveFile`/`showMessageBox`
//! viviam no caminho diferido, cujo único chamador era o sidecar respondendo por `id`. O
//! adaptador `dialog` do SDK (`packages/sdk/src/host/adapters.ts`) era servido pelo host Node, e
//! a única UI que abre arquivo hoje usa `<input type="file">` (`BackupPanel.tsx`). Sem consumidor,
//! o código foi removido em vez de ficar dormente. Reviver = canal no contrato de IPC + chamador
//! de verdade, nessa ordem. Registrado em `docs/tauri-rust-migration.md §5`.

use serde_json::Value;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager};

use crate::protocol::WindowType;
use crate::windows;

/// Evento que o shell EMPURRA sem ninguém ter pedido — atalho global disparado, atualização
/// baixada, tema do SO mudou, lista de janelas mudou.
///
/// Substitui `sidecar::notify`, apagado em 2026-09-22. Ele escrevia no stdin do processo Node, que
/// retransmitia ao renderer; sem esse salto, o evento vai direto às janelas pelo MESMO nome de
/// canal (`updater://available`, `mika://global-shortcut`, `theme://changed`, `windows://changed`),
/// que é o que o renderer já escuta. Uma camada a menos, zero mudança de contrato.
///
/// Broadcast e não `emit_to`: estes eventos não têm janela destinatária — quem se inscreve decide
/// se interessa. `emit_to` com um label fixo perderia o Settings ou o wizard conforme o momento.
pub fn notify(app: &AppHandle, channel: &str, payload: Value) {
    if let Err(error) = app.emit(channel, payload) {
        log::warn!("evento de shell '{channel}' não chegou às janelas: {error}");
    }
}

/// Equivalente a `assertOwnerSurface`: só as superfícies do dono mexem na janela.
/// `plugin-view` e `unknown` são recusadas — um plugin não move a janela do avatar.
pub fn is_owner_surface(window_type: WindowType) -> bool {
    matches!(
        window_type,
        WindowType::Main | WindowType::Settings | WindowType::Wizard
    )
}

/// Maior salto plausível num frame — espelha o schema de `windowMove` em desktop-ipc.ts.
/// Sem esse limite um NaN/Infinity vindo do renderer joga a janela para fora de qualquer monitor.
const MAX_DELTA: f64 = 4000.0;

pub fn move_main_window(app: &AppHandle, delta_x: f64, delta_y: f64) -> Result<(), String> {
    if !delta_x.is_finite() || !delta_y.is_finite() {
        return Err("delta não finito".into());
    }
    if delta_x.abs() > MAX_DELTA || delta_y.abs() > MAX_DELTA {
        return Err("delta fora do limite".into());
    }

    let window = app
        .get_webview_window(windows::MAIN)
        .ok_or("janela principal não existe")?;

    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let position = window
        .outer_position()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);

    window
        .set_position(LogicalPosition::new(
            position.x + delta_x,
            position.y + delta_y,
        ))
        .map_err(|e| e.to_string())
}

/// Native OS move loop for the avatar window; the left button must still be down when this lands.
pub fn start_main_drag(app: &AppHandle) -> Result<(), String> {
    let window = app.get_webview_window(windows::MAIN).ok_or("janela principal não existe")?;
    window.start_dragging().map_err(|e| e.to_string())
}

pub fn set_ignore_mouse(app: &AppHandle, ignore: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(windows::MAIN)
        .ok_or("janela principal não existe")?;

    // Electron tinha `forward: true` para continuar recebendo movimento com clique atravessando.
    // Tauri não expõe esse flag; o movimento vem do polling de cursor (start_cursor_polling),
    // que o app já usava em paralelo — o comportamento do avatar se mantém.
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|e| e.to_string())
}

pub fn toggle_always_on_top(app: &AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window(windows::MAIN)
        .ok_or("janela principal não existe")?;

    // Tauri não tem getter de always-on-top; o estado vive aqui.
    let next = !crate::state::always_on_top(app);
    window
        .set_always_on_top(next)
        .map_err(|e| e.to_string())?;
    crate::state::set_always_on_top(app, next);
    Ok(next)
}

/// `window-close` from `caller`: the avatar only hides; any other surface is destroyed. wry's own
/// `window.close()` destroyed only the webview's container HWND, leaving a dead Tauri window that
/// `create_settings` then re-showed blank instead of building a new one.
pub fn close_window(app: &AppHandle, caller: &str) -> Result<(), String> {
    if hides_instead_of_closing(caller) {
        return hide_main_window(app);
    }
    let window = app.get_webview_window(caller).ok_or_else(|| format!("janela {caller} não existe"))?;
    window.destroy().map_err(|e| e.to_string())
}

fn hides_instead_of_closing(label: &str) -> bool {
    label == windows::MAIN
}

/// Encerra o app de verdade (não só a janela): a MESMA sequência do item "Sair" da bandeja
/// (`tray.rs`) — `quitting` ligado para o avatar não só esconder, e `exit` dispara os shutdown hooks
/// de `lib.rs`. Quem recusa os termos no primeiro uso sai por aqui.
pub fn quit_app(app: &AppHandle) {
    crate::tray::mark_quitting(app);
    app.exit(0);
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(windows::MAIN)
        .ok_or("janela principal não existe")?;
    window.hide().map_err(|e| e.to_string())
}

/// Ações de shell (abrir wizard ao detectar setup pendente, revelar a janela depois do
/// onboarding, abrir uma view de plugin). Chamadores hoje: [`decide_boot`], `bridge.rs` e a
/// bandeja — todos dentro deste processo.
///
/// Ação sem retorno útil responde `null`, que é diferente de "não respondeu".
pub fn handle(app: &AppHandle, action: &str, payload: Value) -> Result<Value, String> {
    let unit = |result: tauri::Result<()>| result.map(|_| Value::Null).map_err(|e| e.to_string());

    match action {
        "openWizard" => unit(windows::create_wizard(app).map(|w| {
            // Decisão de boot tomada: o prazo de segurança não deve revelar o avatar por cima.
            app.state::<crate::state::ShellState>()
                .boot_resolved
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = w.show();
            let _ = w.set_focus();
        })),
        "revealMain" => {
            reveal_main(app);
            Ok(Value::Null)
        }
        "openPluginView" => {
            let plugin_id = payload["pluginId"].as_str().unwrap_or_default();
            let view_id = payload["viewId"].as_str().unwrap_or_default();
            let title = payload["title"].as_str().unwrap_or("Mika");
            let width = payload["width"].as_f64().unwrap_or(1180.0);
            let height = payload["height"].as_f64().unwrap_or(760.0);

            let Some(label) = windows::plugin_view_label(plugin_id, view_id) else {
                return Err("openPluginView exige pluginId e viewId em [a-z0-9.-]".into());
            };

            unit(
                windows::create_plugin_view(app, &label, plugin_id, view_id, title, width, height)
                    .map(|w| {
                        let _ = w.show();
                    }),
            )
        }
        "openExternal" => open_external(payload["url"].as_str().unwrap_or_default()),
        "revealPath" => reveal_path(app, payload["kind"].as_str().unwrap_or_default()),
        // Erro explícito, nunca no-op mudo: ação desconhecida aqui vira um controle da UI
        // que parece ligado e não faz nada.
        other => Err(format!("ação de shell desconhecida: {other}")),
    }
}

/// Onde os links "Obter chave"/"Ajuda" da tela de Configurações podem levar. Allowlist e não "todo
/// https": o renderer é quem pede, e um XSS numa tela de settings não deve virar "abra este site
/// qualquer no navegador do usuário". Link novo = host novo aqui, de propósito.
const EXTERNAL_HOSTS: &[&str] = &[
    "aistudio.google.com",
    "openrouter.ai",
    "console.groq.com",
    "platform.openai.com",
    "portal.azure.com",
    "console.cloud.google.com",
    "elevenlabs.io",
    "ollama.com",
    "unsloth.ai",
    "steamcommunity.com",
    "help.steampowered.com",
    // Templates MCP: token do GitHub, chave do Brave Search, instalação do `uv` e docs dos servidores.
    "github.com",
    "brave.com",
    "docs.astral.sh",
    // `learnMoreUrl` da chave do proxy OpenAI-compatível (`modules/llm/feature_fields.rs`).
    "docs.litellm.ai",
];

fn external_url_allowed(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw).map_err(|_| "URL inválida".to_string())?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err("só https simples é aberto no navegador".into());
    }
    if !EXTERNAL_HOSTS.contains(&host) {
        return Err(format!("host fora da lista de links da Mika: {host}"));
    }
    Ok(url)
}

/// Abre no navegador padrão sem shell (mesma função livre de `features/desktop/launch.rs::open_url`).
fn open_external(raw: &str) -> Result<Value, String> {
    let url = external_url_allowed(raw)?;
    #[allow(deprecated)]
    tauri_plugin_shell::open::open(None, url.as_str(), None).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// Pasta que "Abrir pasta" pode revelar — um nome fechado, nunca um caminho vindo do renderer.
fn reveal_target(data_dir: &std::path::Path, kind: &str) -> Result<std::path::PathBuf, String> {
    match kind {
        "data" => Ok(data_dir.to_path_buf()),
        // Mesmo `<app_data>/logs` que `modules::system::log_files::spawn` grava.
        "logs" => Ok(data_dir.join("logs")),
        other => Err(format!("pasta desconhecida: {other}")),
    }
}

/// Abre a pasta de dados/logs no gerenciador de arquivos do SO e devolve o caminho real, que a UI
/// mostra no lugar do caminho genérico por plataforma.
fn reveal_path(app: &AppHandle, kind: &str) -> Result<Value, String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| e.to_string())?;
    let dir = reveal_target(&data_dir, kind)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[allow(deprecated)]
    tauri_plugin_shell::open::open(None, dir.to_string_lossy(), None).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "path": dir }))
}

/// `[{ label, windowType }]` das janelas vivas. `windowType` vem de `WindowType::from_label`,
/// a mesma função que decide confiança no IPC — uma classificação só, não duas que podem
/// divergir.
pub fn list_windows(app: &AppHandle) -> Value {
    let list: Vec<Value> = app
        .webview_windows()
        .keys()
        .map(|label| {
            serde_json::json!({
                "label": label,
                "windowType": crate::protocol::WindowType::from_label(label),
            })
        })
        .collect();

    Value::Array(list)
}

/// Revela a janela do avatar. Idempotente: o sidecar chama ao decidir que o onboarding
/// não está pendente, e o prazo de segurança chama se o sidecar nunca responder.
pub fn reveal_main(app: &AppHandle) {
    use std::sync::atomic::Ordering;

    let state = app.state::<crate::state::ShellState>();
    state.boot_resolved.store(true, Ordering::Relaxed);

    if state.revealed.swap(true, Ordering::Relaxed) {
        return;
    }

    let Some(window) = app.get_webview_window(windows::MAIN) else {
        return;
    };
    let _ = window.show();
    let _ = window.set_ignore_cursor_events(false);
    let _ = window.set_focus();
    crate::fullscreen::restore(app);
    start_cursor_polling(app);
}

/// **Quem decide o boot desde que o sidecar foi apagado (2026-09-22).**
///
/// Porte de `sidecar/index.ts::decideWizard`, e a decisão inteira sempre foi um `ConfigKey`, não
/// um evento de janela: `setup.wizardCompleted !== true` significa onboarding pendente. Sem este
/// porte, apagar o sidecar deixaria TODO boot esperando os 8 segundos de
/// [`reveal_after_timeout`] e revelando o avatar — o wizard de primeiro uso nunca mais abriria,
/// sem erro e sem log, que é a classe de defeito mais cara desta árvore.
///
/// O `!= Some(true)` é literal do TS (`!== true`): chave ausente conta como PENDENTE. Tratar
/// ausência como concluído puliria o onboarding na primeira execução, que é justamente a única
/// em que ele importa.
pub fn decide_boot(app: &AppHandle) {
    let pending = match crate::state::config_store(app) {
        Ok(store) => store.get("setup.wizardCompleted").and_then(|v| v.as_bool()) != Some(true),
        // Sem store não dá para saber se o onboarding falta. Revelar o avatar esconderia o wizard
        // de quem nunca o viu; abrir o wizard só custa um passo a quem já o concluiu.
        Err(error) => {
            log::error!("boot: ConfigStore indisponível ({error}); assumindo onboarding pendente");
            true
        }
    };

    let action = if pending { "openWizard" } else { "revealMain" };
    if let Err(error) = handle(app, action, Value::Null) {
        log::error!("boot: '{action}' falhou ({error}); o prazo de segurança revela o avatar");
    }
}

/// Janela que nunca aparece é a falha mais cara de diagnosticar: sem erro, sem log, sem tela.
/// Passado o prazo, revela assim mesmo e deixa o motivo no log.
pub fn reveal_after_timeout(app: &AppHandle, seconds: u64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

        use std::sync::atomic::Ordering;
        // `boot_resolved`, não `revealed`: abrir o onboarding TAMBÉM é decidir. Enquanto este
        // teste olhava `revealed`, o prazo revelava o avatar por cima do wizard e o primeiro boot
        // mostrava as duas janelas.
        if app
            .state::<crate::state::ShellState>()
            .boot_resolved
            .load(Ordering::Relaxed)
        {
            return;
        }

        log::warn!("backend não decidiu o boot em {seconds}s; revelando o avatar assim mesmo");
        reveal_main(&app);
    });
}

/// O avatar precisa saber onde o cursor está mesmo com clique atravessando a janela.
/// Mesmo intervalo do `createCursorPoller` do Electron (125ms).
pub fn start_cursor_polling(app: &AppHandle) {
    if crate::state::cursor_polling_running(app) {
        return;
    }
    crate::state::set_cursor_polling(app, true);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // 125ms com a pessoa ativa; o governador de energia espaça ocioso/jogando. Cursor parado
        // (jogo com o mouse preso, ninguém no PC) não gera evento nem render no renderer.
        let base = std::time::Duration::from_millis(125);
        let mut last: Option<Value> = None;
        loop {
            tokio::time::sleep(crate::modules::system::governor::background(base)).await;

            let Some(window) = app.get_webview_window(windows::MAIN) else {
                break;
            };
            if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
                continue;
            }
            let (Ok(cursor), Ok(origin), Ok(size), Ok(scale)) =
                (app.cursor_position(), window.inner_position(), window.inner_size(), window.scale_factor())
            else {
                continue;
            };

            let payload = cursor_payload((cursor.x, cursor.y), (origin.x as f64, origin.y as f64), (size.width as f64, size.height as f64), scale);
            // Fora da janela o renderer descarta o evento (`pollIgnoreDecision`): não vale serializar,
            // cruzar o IPC e acordar o webview a cada 125 ms só porque o cursor mexeu em outro app.
            if payload["isInside"] != true {
                last = None; // reentrar no mesmo pixel tem de reemitir
                continue;
            }
            if last.as_ref() == Some(&payload) {
                continue;
            }
            let _ = window.emit_to(windows::MAIN, MOUSE_MOVE_EVENT, &payload);
            last = Some(payload);
        }
        crate::state::set_cursor_polling(&app, false);
    });
}

/// `DESKTOP_RENDERER_EVENT_CHANNELS.MouseMove`. With click-through on, the webview gets no DOM
/// mouse events, so this poll is the only way the renderer learns the cursor is back over the avatar.
const MOUSE_MOVE_EVENT: &str = "mouse-move";

/// Physical screen cursor → the `{ x, y, insideX, insideY, isInside }` shape `onMouseMove` consumes,
/// with the inside coordinates in CSS pixels so they feed `elementFromPoint` directly.
fn cursor_payload(cursor: (f64, f64), origin: (f64, f64), size: (f64, f64), scale: f64) -> Value {
    let (dx, dy) = (cursor.0 - origin.0, cursor.1 - origin.1);
    let is_inside = dx >= 0.0 && dy >= 0.0 && dx < size.0 && dy < size.1;
    serde_json::json!({
        "x": cursor.0 / scale,
        "y": cursor.1 / scale,
        "insideX": dx / scale,
        "insideY": dy / scale,
        "isInside": is_inside,
    })
}

#[cfg(test)]
mod tests {
    /// Regression: the poll emitted `mika://mouse-move` with `{x, y}` while the bridge listens on
    /// `mouse-move` for `isInside/insideX/insideY`. Once click-through turned on (any blur did it),
    /// nothing turned it off again and the avatar could no longer be clicked or dragged.
    #[test]
    fn cursor_poll_speaks_the_renderer_mouse_move_contract() {
        let contract = include_str!("../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("MouseMove: '{}'", super::MOUSE_MOVE_EVENT)));

        let inside = super::cursor_payload((1300.0, 900.0), (1000.0, 500.0), (800.0, 1280.0), 2.0);
        assert_eq!(inside["isInside"], true);
        assert_eq!(inside["insideX"], 150.0);
        assert_eq!(inside["insideY"], 200.0);

        for cursor in [(999.0, 900.0), (1800.0, 900.0), (1300.0, 1780.0)] {
            let outside = super::cursor_payload(cursor, (1000.0, 500.0), (800.0, 1280.0), 2.0);
            assert_eq!(outside["isInside"], false, "{cursor:?}");
        }
    }

    /// Um link de settings nunca vira "abra qualquer coisa": só https simples para os hosts
    /// nomeados. `file:`/`javascript:`/credencial embutida/porta são recusados antes do SO.
    #[test]
    fn open_external_accepts_only_https_to_the_named_hosts() {
        assert!(super::external_url_allowed("https://console.groq.com/keys").is_ok());
        for bad in [
            "http://console.groq.com/keys",
            "https://evil.example/keys",
            "https://console.groq.com.evil.example/",
            "https://user:pw@console.groq.com/",
            "https://console.groq.com:8443/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "",
        ] {
            assert!(super::external_url_allowed(bad).is_err(), "{bad} deveria ser recusada");
        }
    }

    /// Todo link que a UI abre via `open-external` tem de estar na allowlist — senão o botão "Obter
    /// chave" / "Ver instruções" parece ligado e só devolve "host fora da lista" num toast. Lido do
    /// fonte de cada ponto que declara link (TS e o `learnMoreUrl` das features Rust), porque o
    /// host novo nasce do lado TS e a lista mora deste lado.
    #[test]
    fn every_link_the_ui_opens_is_on_the_open_external_allowlist() {
        let sources: &[(&str, &str)] = &[
            (
                "provider-credential-specs.ts",
                include_str!("../../src/renderer/src/components/settings/provider-credential-specs.ts"),
            ),
            ("mcp-templates.ts", include_str!("../../src/features/mcp/renderer/mcp-templates.ts")),
            (
                "OllamaSetupGuide.tsx",
                include_str!("../../src/renderer/src/components/wizard/OllamaSetupGuide.tsx"),
            ),
            (
                "gamer-companion-steam.tsx",
                include_str!("../../src/features/gamer-companion/renderer/gamer-companion-steam.tsx"),
            ),
            ("llm/feature_fields.rs", include_str!("modules/llm/feature_fields.rs")),
            ("gamer/feature.rs", include_str!("modules/gamer/feature.rs")),
        ];
        let link = regex::Regex::new(r#"(?:keyUrl|docsUrl|learnMoreUrl|_URL)"?\s*[:=]\s*["'](https://[^"']+)"#).unwrap();
        let mut refused = Vec::new();
        for (name, source) in sources {
            let urls: Vec<&str> = link.captures_iter(source).map(|c| c.get(1).unwrap().as_str()).collect();
            assert!(!urls.is_empty(), "{name}: nenhum link encontrado — o teste perdeu o alvo");
            refused.extend(urls.into_iter().filter(|u| super::external_url_allowed(u).is_err()).map(|u| format!("{name}: {u}")));
        }
        assert!(refused.is_empty(), "links fora de EXTERNAL_HOSTS: {refused:#?}");
    }

    /// "Abrir pasta" recebe um NOME, nunca um caminho: nada fora de `<app_data>` é alcançável.
    #[test]
    fn reveal_path_resolves_only_the_named_folders_under_app_data() {
        let base = std::path::Path::new("/perfil");
        assert_eq!(super::reveal_target(base, "data").unwrap(), base);
        assert_eq!(super::reveal_target(base, "logs").unwrap(), base.join("logs"));
        for bad in ["", "../..", "/etc", "cache"] {
            assert!(super::reveal_target(base, bad).is_err(), "{bad} deveria ser recusado");
        }
    }

    #[test]
    fn closing_settings_or_wizard_destroys_it_but_the_avatar_only_hides() {
        assert!(super::hides_instead_of_closing(crate::windows::MAIN));
        assert!(!super::hides_instead_of_closing(crate::windows::SETTINGS));
        assert!(!super::hides_instead_of_closing("wizard"));
    }

    /// Regressão de 2026-09-21, vista no primeiro boot real: o wizard abria e, 8s depois, o avatar
    /// aparecia por cima — as duas janelas ao mesmo tempo. O prazo de segurança testava
    /// `revealed`, que só o `reveal_main` marca, então "abri o onboarding" não contava como
    /// decisão de boot.
    ///
    /// Lido do fonte porque `AppHandle` não existe fora de um app Tauri montado: as duas pontas
    /// deste invariante são duas linhas distantes uma da outra, e é a divergência entre elas que
    /// o defeito era.
    const SOURCE: &str = include_str!("shell_actions.rs");

    fn body_of(function: &str) -> &'static str {
        let start = SOURCE
            .find(function)
            .unwrap_or_else(|| panic!("{function} sumiu de shell_actions.rs — o teste perdeu o alvo"));
        &SOURCE[start..]
    }

    #[test]
    fn boot_timeout_checks_the_decision_flag_not_the_reveal_flag() {
        let body = &body_of("pub fn reveal_after_timeout")[..600];

        assert!(
            body.contains("boot_resolved"),
            "o prazo de segurança precisa olhar `boot_resolved`: abrir o onboarding também é decidir o boot"
        );
        assert!(
            !body.contains(".revealed"),
            "olhar `revealed` faz o prazo revelar o avatar por cima do wizard — foi esse o defeito das duas janelas"
        );
    }

    #[test]
    fn opening_the_wizard_counts_as_deciding_the_boot() {
        let body = &body_of("\"openWizard\"")[..400];

        assert!(
            body.contains("boot_resolved"),
            "`openWizard` precisa marcar `boot_resolved`, senão o prazo revela o avatar por cima do onboarding"
        );
    }

    #[test]
    fn revealing_also_decides_the_boot() {
        let body = &body_of("pub fn reveal_main")[..500];

        assert!(
            body.contains("boot_resolved"),
            "`reveal_main` precisa marcar `boot_resolved` — é o outro braço da mesma decisão"
        );
    }
}
