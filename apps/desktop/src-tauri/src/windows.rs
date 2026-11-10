//! As quatro janelas do app. Os labels espelham `DesktopInvokeContext['windowType']`
//! e são a fonte da verdade do contexto de segurança — ver protocol.rs.

use tauri::webview::{NewWindowFeatures, NewWindowResponse};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const SETTINGS: &str = "settings";
pub const WIZARD: &str = "wizard";

/// Título literal de `create_settings`/`create_wizard` — única fonte de verdade, consultada por
/// `vision::capture_gate` para nunca capturar/analisar a PRÓPRIA UI da Mika (WP9 item 3: "se a
/// janela de configurações da Mika está em primeiro plano, ela deve ser excluída da captura"). O
/// avatar (`create_main`) não entra aqui: nasce sem `.title()` (janela sem moldura), então não tem
/// como uma sonda de SO reportar um título próprio para ele — e não expõe nada sensível como
/// Configurações/Onboarding (chaves de API) expõem.
pub const SETTINGS_TITLE: &str = "Mika — Settings";
pub const WIZARD_TITLE: &str = "Mika — Onboarding";

/// `true` só para os títulos exatos acima — nome de processo (`owner`) não serve para esta checagem
/// porque o executável muda de nome entre dev (`mika-desktop.exe`, `Cargo.toml`) e release
/// (`Mika.exe`, `productName` do `tauri.conf.json`); o título da janela é o único sinal estável nos
/// dois builds.
pub fn is_own_window_title(title: &str) -> bool {
    title == SETTINGS_TITLE || title == WIZARD_TITLE
}

/// Porta do Vite em dev (`devUrl` do `tauri.conf.json`).
const DEV_SERVER_PORT: u16 = 5273;

/// Para onde um webview do app PODE navegar: só o próprio bundle (`tauri://…`, ou
/// `http(s)://tauri.localhost` no Windows, sem porta) e, em dev, o servidor do Vite. Todo o resto — um
/// `<a href>`/`location=` num texto que a Mika renderizou, um redirect, `file:`/`javascript:` — fica
/// no lugar: sem isto, uma navegação de topo colocava um site de terceiros DENTRO da janela, com a
/// ponte `window.electronAPI` (IPC do app) ao alcance dele. Links externos passam por
/// `open-external` (allowlist de hosts), nunca pelo webview.
pub(crate) fn navigation_allowed(url: &Url, dev: bool) -> bool {
    match url.scheme() {
        "tauri" => true,
        "http" | "https" if url.host_str() == Some("tauri.localhost") => url.port().is_none(),
        "http" => dev && url.host_str() == Some("localhost") && url.port() == Some(DEV_SERVER_PORT),
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

fn navigation_guard() -> impl Fn(&Url) -> bool + Send + 'static {
    let dev = tauri::is_dev();
    move |url| {
        let allowed = navigation_allowed(url, dev);
        if !allowed {
            log::warn!("navegação do webview bloqueada (scheme={}, host={:?})", url.scheme(), url.host_str());
        }
        allowed
    }
}

/// `window.open`/`target=_blank` nunca abre janela nova do app: janela filha herdaria a ponte IPC.
fn deny_new_window(url: Url, _features: NewWindowFeatures) -> NewWindowResponse<tauri::Wry> {
    log::warn!("window.open bloqueado (scheme={}, host={:?})", url.scheme(), url.host_str());
    NewWindowResponse::Deny
}

/// O backend enxerga as janelas por um cache (`BrowserWindow.getAllWindows()` do Electron é
/// SÍNCRONO e o dado mora aqui). Cache só é honesto se for empurrado: toda janela que nasce ou
/// morre avisa. Sem isto, um evento de renderer seria mandado para uma janela já fechada — ou,
/// pior, deixaria de alcançar uma recém-aberta, em silêncio.
pub fn announce(app: &AppHandle) {
    crate::shell_actions::notify(app, "windows://changed", crate::shell_actions::list_windows(app));
}

/// Registra o aviso de fechamento, instala a guarda de permissão e anuncia a janela nova.
fn track(app: &AppHandle, window: &WebviewWindow) {
    crate::permissions::install(window);
    let closing = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            announce(&closing);
        }
    });
    announce(app);
}

/// O que a janela `label` carrega. Com o host HTTP do E2E (`e2e_host.rs`, só na feature `e2e`) a UI
/// roda no Chromium do Playwright e a janela nativa fica em `about:blank`.
fn page(label: &str, path: &str) -> WebviewUrl {
    #[cfg(feature = "e2e")]
    if crate::e2e_host::active() {
        return crate::e2e_host::page(label, path);
    }
    let _ = label;
    WebviewUrl::App(path.into())
}

/// Espelha AVATAR_WINDOW em src/main/windows/main-window.ts.
const AVATAR: Size = Size {
    width: 400.0,
    height: 640.0,
    min_width: 300.0,
    min_height: 400.0,
};

struct Size {
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

/// O preload lê `process.platform` e `process.versions` de forma síncrona, na construção da
/// API. No Electron isso vinha do Node do processo de preload; aqui não há Node no webview,
/// e sem o shim o boot lança `process is not defined` e a janela sobe em branco.
///
/// Roda antes de qualquer script da página — é o slot equivalente ao preload.
///
/// `node` e `electron` são DEIXADOS DE FORA de propósito, não preenchidos com string vazia:
/// `live2dcubismcore.min.js` é emscripten e decide se está no Node com
/// `typeof process.versions.node == "string"`. Uma string vazia satisfaz esse teste e o faz
/// chamar `require('fs')` — o avatar morreria no carregamento, e o rastro apontaria para o
/// Live2D, não para este shim.
///
/// **Nada aqui é congelado, e isso não é descuido — `Object.freeze` deixava a janela BRANCA.**
///
/// O cliente de dev do Vite (`vite/dist/client/env.mjs`) percorre TODA chave de `define` e a
/// atribui em runtime, caminhando o caminho pontuado: `process.env.NODE_ENV`, `process.platform`,
/// `process.versions.node`, `.chrome`, `.electron` — todos declarados em
/// `apps/desktop/vite.tauri.config.ts`. Como este shim roda ANTES, cada `Object.freeze` virava uma
/// parede: primeiro `TypeError: Cannot add property NODE_ENV, object is not extensible`, e, com
/// `env` liberado, `Cannot assign to read only property 'platform'`. ESM é strict, então a escrita
/// não falha em silêncio: mata o módulo, o React nunca monta, e a janela sobe branca — o MESMO
/// sintoma que este shim existe para evitar, pela causa oposta.
///
/// Nenhum gate via: só o caminho de DEV carrega o cliente do Vite, e 1351 testes de unidade
/// passavam com o app impossível de abrir.
///
/// A ausência de `versions.node` continua garantida, e agora por dois lados: este shim não declara
/// a chave, e em dev o Vite grava `process.versions.node = undefined` — `typeof undefined` é
/// `"undefined"`, nunca `"string"`, que é exatamente o teste do `live2dcubismcore.min.js`. O freeze
/// nunca foi o que dava essa garantia; a ausência do valor é.
/// wry's `window.close()` destroys only the webview's container HWND and leaves the Tauri window
/// alive and blank; `window-close` closes the caller for real (`shell_actions::close_window`).
const OWNER_CLOSE_SHIM: &str =
    "window.close = () => window.__TAURI_INTERNALS__.invoke('mika_invoke', { channel: 'window-close' });";

fn runtime_shim() -> String {
    // `process.platform` usa os valores do Node ('win32'/'darwin'/'linux'), não os do Rust.
    let platform = match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    };
    let webview = tauri::webview_version().unwrap_or_default();

    format!(
        r#"globalThis.process = {{
  platform: {platform:?},
  env: {{}},
  versions: {{ chrome: {webview:?} }},
}};"#
    )
}

/// A janela do avatar: sem moldura, transparente e sempre no topo.
/// Nasce escondida — quem revela é o `ready-to-show`, ou o wizard ao fechar.
pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, MAIN, page(MAIN, "index.html"))
        .initialization_script(&runtime_shim())
        .on_navigation(navigation_guard())
        .on_new_window(deny_new_window)
        .initialization_script(OWNER_CLOSE_SHIM)
        .inner_size(AVATAR.width, AVATAR.height)
        .min_inner_size(AVATAR.min_width, AVATAR.min_height)
        .decorations(false)
        .transparent(true)
        // Tauri defaults to `shadow: true`, which on Windows paints a 1px border and rounds the corners.
        .shadow(false)
        .always_on_top(true)
        .visible(false)
        .resizable(true)
        .center()
        .build()?;

    // Um único handler para os dois eventos: registrar `on_window_event` duas vezes na mesma
    // janela não é contrato garantido do Tauri, e um dos dois avisos sumiria sem erro.
    //
    // O tema do SO só chega como evento de JANELA — o sidecar não tem uma, então é daqui que
    // `nativeTheme.on('updated')` passa a existir de verdade, em vez de um poll em background.
    let owner = app.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::ThemeChanged(theme) => {
            let dark = matches!(theme, tauri::Theme::Dark);
            crate::shell_actions::notify(&owner, "theme://changed", serde_json::json!({ "dark": dark }));
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            crate::events::bus().publish(crate::events::EventKey::ThemeChanged, serde_json::json!({ "isDark": dark, "at": at }));
        }
        tauri::WindowEvent::Resized(_) => crate::fullscreen::on_resized(&owner),
        tauri::WindowEvent::Destroyed => announce(&owner),
        _ => {}
    });
    // A janela do avatar não passa por `track` (tem handler próprio), mas precisa da MESMA
    // guarda — é justamente ela que pede o microfone.
    crate::permissions::install(&window);
    announce(app);

    Ok(window)
}

pub fn create_settings(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(SETTINGS) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(existing);
    }

    // 80% da área útil, com 1280x800 de piso — igual ao settings-window.ts.
    let (width, height) = fraction_of_work_area(app, 0.8).unwrap_or((1280.0, 800.0));

    let window = WebviewWindowBuilder::new(app, SETTINGS, page(SETTINGS, "index.html#/settings"))
        .initialization_script(&runtime_shim())
        .on_navigation(navigation_guard())
        .on_new_window(deny_new_window)
        .initialization_script(OWNER_CLOSE_SHIM)
        .title(SETTINGS_TITLE)
        .inner_size(width, height)
        .min_inner_size(960.0, 640.0)
        .visible(false)
        .resizable(true)
        .center()
        .build()?;

    track(app, &window);
    Ok(window)
}

pub fn create_wizard(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(WIZARD) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(existing);
    }

    let window = WebviewWindowBuilder::new(app, WIZARD, page(WIZARD, "index.html#/wizard"))
        .initialization_script(&runtime_shim())
        .on_navigation(navigation_guard())
        .on_new_window(deny_new_window)
        .initialization_script(OWNER_CLOSE_SHIM)
        .title(WIZARD_TITLE)
        .inner_size(1280.0, 860.0)
        .min_inner_size(960.0, 700.0)
        .visible(false)
        .resizable(true)
        .center()
        .build()?;

    track(app, &window);
    Ok(window)
}

/// Label de janela no Tauri só aceita `[A-Za-z0-9-/:_]`; ids de plugin e de view são `[a-z0-9.-]`
/// (sem `_`), então trocar `.` por `_` é reversível. `None` recusa id fora do alfabeto — o mesmo id
/// vai para a query string da janela.
pub fn plugin_view_label(plugin_id: &str, view_id: &str) -> Option<String> {
    let valid = |id: &str| !id.is_empty() && id.len() <= 120 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-'));
    (valid(plugin_id) && valid(view_id)).then(|| format!("plugin-view::{}::{}", plugin_id.replace('.', "_"), view_id.replace('.', "_")))
}
/// Inverso de [`plugin_view_label`] para o id do plugin.
pub fn plugin_id_from_label(label: &str) -> Option<String> {
    label.strip_prefix("plugin-view::")?.split("::").next().map(|id| id.replace('_', "."))
}
/// Janela de view de plugin. O label carrega o id do plugin (`plugin-view::<id>::<view>`)
/// porque é dele que `WindowType::from_label` e o `pluginId` do contexto são derivados.
pub fn create_plugin_view(
    app: &AppHandle,
    label: &str,
    plugin_id: &str,
    view_id: &str,
    title: &str,
    width: f64,
    height: f64,
) -> tauri::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(existing);
    }

    let url = format!("index.html?type=plugin-view&pluginId={plugin_id}&viewId={view_id}");

    let window = WebviewWindowBuilder::new(app, label, page(label, &url))
        .initialization_script(&runtime_shim())
        .on_navigation(navigation_guard())
        .on_new_window(deny_new_window)
        .title(title)
        .inner_size(width, height)
        .min_inner_size(width.min(900.0), height.min(620.0))
        .visible(false)
        .resizable(true)
        .center()
        .build()?;

    track(app, &window);
    Ok(window)
}

fn fraction_of_work_area(app: &AppHandle, fraction: f64) -> Option<(f64, f64)> {
    let monitor = app.primary_monitor().ok()??;
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    Some((
        (size.width * fraction).round(),
        (size.height * fraction).round(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WP9 item 3: só os títulos exatos de Configurações/Onboarding contam como "própria Mika" —
    /// nunca uma janela de terceiros que por acaso contenha "Mika" no título (ex. uma aba de
    /// navegador pesquisando pelo nome do produto).
    #[test]
    fn is_own_window_title_matches_only_the_settings_and_wizard_titles() {
        assert!(is_own_window_title(SETTINGS_TITLE));
        assert!(is_own_window_title(WIZARD_TITLE));
        assert!(!is_own_window_title("Mika — Wikipedia"));
        assert!(!is_own_window_title(""));
    }

    /// O Tauri recusa `.` em label de janela; `mika.gamer-companion` derrubava toda view de plugin.
    #[test]
    fn plugin_view_label_is_tauri_safe_and_round_trips_the_plugin_id() {
        let label = plugin_view_label("mika.gamer-companion", "gamer-companion.mini-games").unwrap();
        assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_')), "{label}");
        assert_eq!(plugin_id_from_label(&label).as_deref(), Some("mika.gamer-companion"));
        assert_eq!(crate::protocol::WindowType::from_label(&label), crate::protocol::WindowType::PluginView);
    }

    #[test]
    fn plugin_view_label_refuses_ids_outside_the_alphabet() {
        for (plugin, view) in [("", "main"), ("mika.x", ""), ("mika_x", "main"), ("mika.x", "a&b=c"), ("Mika.X", "main")] {
            assert!(plugin_view_label(plugin, view).is_none(), "{plugin} / {view}");
        }
    }

    /// The avatar window must be borderless: with the default shadow, DWM draws a 1px frame around
    /// the transparent window on Windows.
    #[test]
    fn the_avatar_window_opts_out_of_the_frame_shadow() {
        let source = include_str!("windows.rs");
        let main = &source[source.find("pub fn create_main").unwrap()..source.find("pub fn create_settings").unwrap()];
        assert!(main.contains(".transparent(true)") && main.contains(".shadow(false)"), "create_main lost transparent/shadow(false)");
    }

    /// O shim é injetado ANTES de qualquer script da página, inclusive o cliente de dev do Vite —
    /// que grava `process.env.NODE_ENV`. Congelar `env` fazia esse módulo lançar
    /// `TypeError: Cannot add property NODE_ENV, object is not extensible` e a janela subir BRANCA,
    /// com o app impossível de abrir e a suíte inteira verde: nenhum teste de unidade carrega o
    /// cliente do Vite, e o erro só existe no caminho de DEV.
    ///
    /// Este teste lê o SHIM GERADO, não o fonte: é o texto que chega ao webview.
    #[test]
    fn the_shim_never_freezes_anything_the_vite_dev_client_has_to_write() {
        let shim = runtime_shim();
        assert!(
            !shim.contains("Object.freeze"),
            "voltou `Object.freeze` ao shim — o cliente de dev do Vite atribui process.env.NODE_ENV, \
             process.platform e process.versions.* em runtime, e cada freeze vira um TypeError que \
             mata o módulo: a janela sobe BRANCA e nenhum teste de unidade vê.\n{shim}"
        );
        assert!(shim.contains("env: {}"), "process.env sumiu do shim — o define de NODE_ENV não teria onde pousar:\n{shim}");
    }

    /// A outra metade do contrato, que descongelar NÃO podia afrouxar: nenhuma chave `node` em
    /// `versions`. `live2dcubismcore.min.js` testa `typeof process.versions.node == "string"` e, se
    /// passar, chama `require('fs')` — o avatar morre no carregamento e o rastro culpa o Live2D.
    /// Quem garante isso é a AUSÊNCIA do valor, nunca o freeze: em dev o próprio Vite grava
    /// `versions.node = undefined`, e `typeof undefined` não é `"string"`.
    #[test]
    fn the_shim_never_advertises_a_node_runtime() {
        let shim = runtime_shim();
        assert!(!shim.contains("node:"), "`versions.node` voltou ao shim — o Live2D vai tentar require('fs'):\n{shim}");
        assert!(!shim.contains("electron:"), "`versions.electron` voltou ao shim:\n{shim}");
        assert!(shim.contains("versions: {"), "`versions` sumiu do shim — o preload lê essa chave na construção da API:\n{shim}");
    }

    /// `platform` tem de falar a língua do Node (`win32`/`darwin`), não a do Rust (`windows`/
    /// `macos`): o preload e o código do renderer comparam com os valores do Node.
    #[test]
    fn the_shim_reports_a_node_style_platform() {
        let shim = runtime_shim();
        assert!(!shim.contains("\"windows\""), "platform saiu como o literal do Rust, não o do Node:\n{shim}");
        assert!(!shim.contains("\"macos\""), "platform saiu como o literal do Rust, não o do Node:\n{shim}");
    }

    fn nav(url: &str, dev: bool) -> bool {
        navigation_allowed(&Url::parse(url).unwrap(), dev)
    }

    #[test]
    fn webviews_may_only_navigate_inside_the_bundle() {
        for url in [
            "tauri://localhost/index.html",
            "tauri://localhost/index.html#/settings",
            "http://tauri.localhost/index.html?type=plugin-view&pluginId=a&viewId=b",
            "https://tauri.localhost/",
            "about:blank",
        ] {
            assert!(nav(url, false), "{url} é o próprio app");
        }
        for url in [
            "https://evil.example/",
            "http://evil.example/tauri.localhost",
            "http://tauri.localhost.evil.example/",
            "https://tauri.localhost@evil.example/",
            "http://localhost:5273/",
            "http://localhost:8080/",
            "http://127.0.0.1:5273/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "blob:https://x/y",
            "ftp://tauri.localhost/",
            "about:srcdoc",
            // Outro serviço no `tauri.localhost` de uma porta qualquer não é o bundle do app.
            "http://tauri.localhost:8080/",
            "https://tauri.localhost:5273/index.html",
        ] {
            assert!(!nav(url, false), "{url} não pode navegar");
        }
    }

    /// O servidor do Vite só vale em dev e só na porta do `devUrl`.
    #[test]
    fn the_vite_dev_server_is_allowed_only_in_dev_and_only_on_its_port() {
        assert!(nav("http://localhost:5273/index.html", true));
        assert!(!nav("http://localhost:5273/index.html", false));
        assert!(!nav("http://localhost:3000/", true));
        assert!(!nav("https://localhost:5273/", true));
        assert!(!nav("http://localhost/", true));
    }

    /// Falha de OMISSÃO: uma quinta janela criada sem a guarda compilaria e passaria no resto.
    #[test]
    fn every_window_builder_installs_the_navigation_guard_and_denies_new_windows() {
        let source = include_str!("windows.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        let builders = production.matches("WebviewWindowBuilder::new(").count();
        assert_eq!(builders, 4, "o número de janelas mudou — confira cada uma");
        assert_eq!(production.matches(".on_navigation(navigation_guard())").count(), builders);
        assert_eq!(production.matches(".on_new_window(deny_new_window)").count(), builders);
    }
}
