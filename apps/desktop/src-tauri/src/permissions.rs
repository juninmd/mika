//! Guarda de permissão de mídia do webview — o equivalente Tauri de
//! `session.setPermissionRequestHandler` (ver `src/main/security/media-permissions.ts`).
//!
//! **Por que existe.** Sob Electron a decisão era silenciosa e determinística: origem da própria
//! app ganha microfone, qualquer outra é negada sem perguntar. Sem este módulo, quem decide é o
//! prompt nativo do WebView2/WKWebView/WebKitGTK, que PERGUNTA AO USUÁRIO para qualquer origem —
//! inclusive uma hipoteticamente maliciosa que chegasse a carregar no mesmo webview. CSP
//! (`default-src 'self'`) e a ausência de rota de navegação externa mitigam em profundidade, mas
//! mitigação não é a negação determinística que existia antes. Isso era perda de feature, não
//! detalhe de porte.
//!
//! **A regra não podia ser portada literalmente.** `isAppOrigin` do Electron aceitava
//! `http://localhost` e os esquemas `app:`/`mika-desktop:`/`file:`. Sob Tauri a origem do
//! documento é `http://tauri.localhost` (Windows) ou `tauri://localhost` (macOS/Linux) — nenhuma
//! das duas casa com aquela lista. Copiar a regra como estava negaria o microfone da PRÓPRIA
//! Mika, que é a feature central. As duas origens entraram na lista dos dois lados, e
//! `permissions::contract` prova que as listas continuam iguais lendo o fonte TypeScript.
//!
//! Casamento de host é EXATO, nunca por prefixo ou sufixo: `evil.tauri.localhost` e
//! `localhost.evil.com` são origens de terceiros e continuam negadas.

/// Espelha `APP_SCHEMES` em `src/main/security/media-permissions.ts` (sem o `:` final).
#[cfg(any(target_os = "windows", test))]
const APP_SCHEMES: [&str; 4] = ["app", "mika-desktop", "file", "tauri"];

/// Espelha `APP_HOSTS` do mesmo arquivo. Só vale para `http`/`https`.
#[cfg(any(target_os = "windows", test))]
const APP_HOSTS: [&str; 2] = ["localhost", "tauri.localhost"];

/// A origem é a da própria aplicação?
///
/// Origem ausente/vazia é da própria app: o WebView2 não manda URI para requisição que nasce do
/// próprio documento carregado por esquema customizado. Mesma decisão do `isAppOrigin` do TS,
/// que devolve `true` para `undefined`.
#[cfg(any(target_os = "windows", test))]
pub fn is_app_origin(origin: &str) -> bool {
    if origin.is_empty() {
        return true;
    }

    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };

    if APP_SCHEMES.contains(&url.scheme()) {
        return true;
    }

    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| APP_HOSTS.contains(&host))
}

/// Origin whose microphone is pre-granted in the webview profile.
///
/// A permission prompt ignored 3+ times puts the origin under Chromium's embargo: from then on the
/// request is denied WITHOUT firing `PermissionRequested`, so the guard above never decides. It hit
/// `http://localhost:5273` (4 prompts ignored before the guard existed): mic denied and
/// `enumerateDevices` hiding microphones AND speakers. An explicit ALLOW setting beats the embargo,
/// and it is the same decision the guard already makes for the app origin.
#[cfg(any(target_os = "windows", test))]
pub fn granted_media_origin(url: &url::Url) -> Option<String> {
    let origin = url.origin();
    if !origin.is_tuple() {
        return None;
    }
    let origin = origin.ascii_serialization();
    is_app_origin(&origin).then_some(origin)
}

#[cfg(target_os = "windows")]
mod platform {
    use tauri::{Manager, WebviewWindow};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND_CAMERA, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
        COREWEBVIEW2_PERMISSION_STATE, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
        COREWEBVIEW2_PERMISSION_STATE_DENY, ICoreWebView2, ICoreWebView2_13, ICoreWebView2PermissionRequestedEventArgs,
        ICoreWebView2Profile4,
    };
    use webview2_com::{PermissionRequestedEventHandler, SetPermissionStateCompletedHandler, take_pwstr};
    use windows_core::{HSTRING, Interface, PWSTR};

    /// Mídia é microfone e câmera — o mesmo conjunto que `MEDIA` cobre no TS (`media`,
    /// `microphone`, `audioCapture`). Todo o RESTO (geolocalização, notificações, clipboard,
    /// sensores, downloads múltiplos) é negado: nenhum caminho do produto pede essas, e o
    /// Electron também as negava, já que só `MEDIA` podia ser permitida.
    fn state_for(args: &ICoreWebView2PermissionRequestedEventArgs) -> COREWEBVIEW2_PERMISSION_STATE {
        let mut kind = COREWEBVIEW2_PERMISSION_KIND_MICROPHONE;
        if unsafe { args.PermissionKind(&mut kind) }.is_err() {
            return COREWEBVIEW2_PERMISSION_STATE_DENY;
        }
        let is_media =
            kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE || kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA;
        if !is_media {
            return COREWEBVIEW2_PERMISSION_STATE_DENY;
        }

        let mut raw = PWSTR::null();
        // Falha ao ler a URI não vira "sem origem" (que seria PERMITIR): sem saber quem pede,
        // nega. Negar por omissão, como `WindowType::from_label` já faz no IPC.
        if unsafe { args.Uri(&mut raw as *mut PWSTR) }.is_err() || raw.is_null() {
            return COREWEBVIEW2_PERMISSION_STATE_DENY;
        }
        // `take_pwstr`, não `to_string()`: a string vem de `CoTaskMemAlloc` e alguém precisa
        // liberá-la. Sem isso, cada pedido de permissão vaza alguns bytes para sempre.
        let origin = take_pwstr(raw);

        if super::is_app_origin(&origin) {
            COREWEBVIEW2_PERMISSION_STATE_ALLOW
        } else {
            log::warn!("permissão de mídia negada para origem de terceiro: {origin}");
            COREWEBVIEW2_PERMISSION_STATE_DENY
        }
    }

    /// Stores a microphone ALLOW for the app origin in the profile (see `granted_media_origin`).
    fn pre_grant_microphone(core: &ICoreWebView2, origin: &str, label: &str) {
        let profile = core.cast::<ICoreWebView2_13>().and_then(|c| unsafe { c.Profile() });
        let Ok(profile) = profile.and_then(|p| p.cast::<ICoreWebView2Profile4>()) else {
            log::warn!("microfone sem pré-concessão em \"{label}\": runtime do WebView2 sem ICoreWebView2Profile4");
            return;
        };
        let (owner, granted) = (label.to_string(), origin.to_string());
        let done = SetPermissionStateCompletedHandler::create(Box::new(move |result| {
            match result {
                Ok(()) => log::info!("microfone pré-concedido a {granted} em \"{owner}\""),
                Err(error) => log::error!("pré-concessão do microfone falhou em \"{owner}\": {error}"),
            }
            Ok(())
        }));
        let state = COREWEBVIEW2_PERMISSION_STATE_ALLOW;
        let kind = COREWEBVIEW2_PERMISSION_KIND_MICROPHONE;
        if let Err(error) = unsafe { profile.SetPermissionState(kind, &HSTRING::from(origin), state, &done) } {
            log::error!("pré-concessão do microfone falhou em \"{label}\": {error}");
        }
    }

    pub fn install(window: &WebviewWindow) {
        let label = window.label().to_string();
        // `window.url()` is still `about:blank` here: take the origin from config, as Tauri builds it
        // (devUrl in dev; `http://tauri.localhost` serving the bundle on Windows).
        let dev_url = tauri::is_dev().then(|| window.config().build.dev_url.clone()).flatten();
        let base = dev_url.or_else(|| url::Url::parse("http://tauri.localhost").ok());
        let granted = base.as_ref().and_then(super::granted_media_origin);
        if let Err(error) = window.with_webview(move |webview| {
            let controller = webview.controller();
            let Ok(core) = (unsafe { controller.CoreWebView2() }) else {
                log::error!("permissões de mídia sem guarda em \"{label}\": CoreWebView2 indisponível");
                return;
            };

            let mut token = 0i64;
            let handler = PermissionRequestedEventHandler::create(Box::new(|_webview, args| {
                // Sem `args` não há o que responder; o WebView2 então aplica o próprio padrão
                // (perguntar). Nada a fazer aqui além de não fingir que decidimos.
                if let Some(args) = args {
                    unsafe { args.SetState(state_for(&args)) }.ok();
                }
                Ok(())
            }));

            if let Err(error) = unsafe { core.add_PermissionRequested(&handler, &mut token) } {
                log::error!("permissões de mídia sem guarda em \"{label}\": {error}");
            }
            if let Some(origin) = granted {
                pre_grant_microphone(&core, &origin, &label);
            }
        }) {
            log::error!("permissões de mídia sem guarda: with_webview falhou: {error}");
        }
    }
}

/// macOS (`WKUIDelegate.decidePolicyForMediaCapturePermission`) e Linux (sinal
/// `permission-request` do WebKitGTK) precisam da mesma instalação, com as dependências de
/// plataforma correspondentes. Não foram escritos aqui porque não há como compilar nem executar
/// nenhum dos dois nesta máquina, e guarda de segurança não verificada é pior que ausência
/// declarada: o log abaixo diz, em toda abertura de janela, que o prompt nativo é quem decide.
#[cfg(not(target_os = "windows"))]
mod platform {
    use tauri::WebviewWindow;

    pub fn install(window: &WebviewWindow) {
        log::warn!(
            "permissões de mídia em \"{}\" decididas pelo prompt nativo: guarda por origem só implementada no Windows (ver permissions.rs)",
            window.label()
        );
    }
}

pub use platform::install;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_the_app_own_origins() {
        for origin in [
            "",
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "http://localhost:5273",
            "app://index.html",
            "file:///C:/app/index.html",
        ] {
            assert!(is_app_origin(origin), "deveria permitir {origin}");
        }
    }

    #[test]
    fn denies_third_party_origins() {
        for origin in [
            "https://evil.com",
            // Sufixo e prefixo do host da app não herdam o microfone.
            "http://localhost.evil.com",
            "http://evil.tauri.localhost",
            "https://tauri.localhost.evil.com",
            // Não é URL: sem origem legível, nega.
            "nao-e-url",
        ] {
            assert!(!is_app_origin(origin), "deveria negar {origin}");
        }
    }

    #[test]
    fn pre_grants_the_microphone_only_to_the_app_own_tuple_origin() {
        let origin = |raw: &str| granted_media_origin(&url::Url::parse(raw).unwrap());
        assert_eq!(origin("http://localhost:5273/index.html#/settings").as_deref(), Some("http://localhost:5273"));
        assert_eq!(origin("http://tauri.localhost/").as_deref(), Some("http://tauri.localhost"));
        assert_eq!(origin("https://evil.com/"), None);
        assert_eq!(origin("http://evil.tauri.localhost/"), None);
        // Non-special schemes have an opaque ("null") origin: nothing to pre-grant.
        assert_eq!(origin("tauri://localhost/"), None);
    }

    /// `security/media-permissions.ts` foi removido (backend Node deletado na migração para
    /// Tauri) — as duas tabelas de origem confiável que ele fixava (`APP_SCHEMES`/`APP_HOSTS`)
    /// viram snapshot congelado, extraído do fonte no momento da deleção. Divergir de um literal
    /// continua sendo exatamente a classe de furo que este módulo existe para fechar — só que sem
    /// um segundo lado TS para comparar em runtime (a leitura de disco era, além disso, um risco:
    /// falhava a suíte inteira se o arquivo não estivesse ao lado do binário).
    #[test]
    fn origin_lists_match_the_frozen_typescript_snapshot() {
        let mut frozen_schemes = ["app", "mika-desktop", "file", "tauri"];
        let mut frozen_hosts = ["localhost", "tauri.localhost"];
        let mut rust_schemes = APP_SCHEMES;
        let mut rust_hosts = APP_HOSTS;
        frozen_schemes.sort();
        frozen_hosts.sort();
        rust_schemes.sort();
        rust_hosts.sort();

        assert_eq!(rust_schemes, frozen_schemes, "APP_SCHEMES divergiu do snapshot congelado do TypeScript");
        assert_eq!(rust_hosts, frozen_hosts, "APP_HOSTS divergiu do snapshot congelado do TypeScript");
    }
}
