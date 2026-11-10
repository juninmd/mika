//! Esquema Tauri custom `mika-live2d`, servindo APENAS arquivos sob `<app_data_dir>/live2d/` — o
//! local que `LICENSE-ASSETS.md` §"Como usar Live2D numa instalação" documenta para quem instala
//! manualmente uma amostra Live2D, já que esses assets de terceiro saem do instalador
//! (`vite.tauri.config.ts::closeBundle`). Registrar um esquema (em vez de alargar
//! `assetProtocol.scope`, que é estático em `tauri.conf.json` e NUNCA acompanharia
//! `MIKA_PROFILE_DIR` em runtime — ver `state::app_data_dir`) é o único jeito de servir o perfil
//! certo tanto num usuário real quanto no E2E.
//!
//! Mapeamento de plataforma do próprio Tauri (`Builder::register_uri_scheme_protocol`):
//! Windows/Android → `http://mika-live2d.localhost/<path>`; macOS/iOS/Linux →
//! `mika-live2d://localhost/<path>`. Os dois formatos chegam aqui com o mesmo `request.uri().path()`.

use std::path::{Component, Path, PathBuf};

use tauri::http::{
    header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE, ORIGIN},
    Method, Request, Response, StatusCode,
};
use tauri::{AppHandle, Runtime, UriSchemeContext};

pub const SCHEME: &str = "mika-live2d";

#[derive(Debug, PartialEq, Eq)]
enum PathError {
    /// Vazio, `..`, absoluto, prefixo de drive, NUL, ou escapa da raiz depois de canonicalizar
    /// (symlink incluído) — todo caso de fuga cai no mesmo 404 nomeado, nunca num erro que
    /// distinga "caminho ruim" de "arquivo ausente" para quem está do outro lado da rede.
    Escaped,
}

/// Núcleo puro e testável: decodifica percent-encoding, valida e resolve `request_path` dentro
/// de `root`. `root` é canonicalizado aqui para a checagem `starts_with` sobreviver a symlink —
/// sem isso um `live2d/link-pro-fora -> C:\` no perfil do usuário escaparia da pasta.
fn resolve_asset_path(root: &Path, request_path: &str) -> Result<PathBuf, PathError> {
    let decoded = percent_decode(request_path);
    let relative = decoded.trim_start_matches('/');
    if relative.is_empty() || relative.contains('\0') {
        return Err(PathError::Escaped);
    }

    let candidate = Path::new(relative);
    for component in candidate.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(PathError::Escaped);
        }
    }

    let canonical_root = root.canonicalize().map_err(|_| PathError::Escaped)?;
    let canonical_joined = canonical_root.join(candidate).canonicalize().map_err(|_| PathError::Escaped)?;
    if !canonical_joined.starts_with(&canonical_root) {
        return Err(PathError::Escaped);
    }
    Ok(canonical_joined)
}

/// Decodificação percent-encoding manual (só `%XX` ASCII) — evita puxar um crate novo para uma
/// função de ~10 linhas. Cobre o caso que importa: `%2e%2e` vira `..` ANTES da checagem de
/// componente, então um `..` disfarçado é barrado como qualquer outro.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `.model3.json`/`.motion3.json`/`.exp3.json`/`.physics3.json`/`.cdi3.json`/`.pose3.json` são
/// todos JSON puro com sufixo composto — `Path::extension()` já devolve só o último segmento
/// (`json`), então um match na extensão final cobre a família inteira sem listar cada uma.
fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => "application/json",
        Some("png") => "image/png",
        // `.moc3` e qualquer outra extensão (ex. o diretório de textura `*.2048/`, que nunca é
        // servido diretamente — só os `.png` dentro dele) caem no genérico.
        _ => "application/octet-stream",
    }
}

fn live2d_root<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
    Ok(crate::state::app_data_dir(app)?.join("live2d"))
}

/// The app's own origins (WebView2 on Windows, WKWebView/WebKitGTK elsewhere). The model loader
/// fetches cross-origin via XHR, so without an echoed origin every request is blocked by CORS.
const APP_ORIGINS: [&str; 2] = ["http://tauri.localhost", "tauri://localhost"];

fn allowed_origin(request_origin: Option<&str>) -> Option<&'static str> {
    APP_ORIGINS.into_iter().find(|origin| Some(*origin) == request_origin)
}

fn respond(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn with_cors(mut response: Response<Vec<u8>>, request_origin: Option<&str>) -> Response<Vec<u8>> {
    if let Some(origin) = allowed_origin(request_origin) {
        response.headers_mut().insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin.parse().expect("static origin"));
    }
    response
}

/// Handler registrado em `lib.rs` via `Builder::register_uri_scheme_protocol(SCHEME, ...)`.
/// GET only; qualquer falha (raiz ausente, caminho fora da raiz, arquivo inexistente) vira 404 —
/// nunca 500, para não vazar se o problema foi o pedido ou o disco.
pub fn handler<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let origin = request.headers().get(ORIGIN).and_then(|value| value.to_str().ok()).map(str::to_owned);
    with_cors(serve(&ctx, &request), origin.as_deref())
}

fn serve<R: Runtime>(ctx: &UriSchemeContext<'_, R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return respond(StatusCode::METHOD_NOT_ALLOWED, "text/plain", Vec::new());
    }
    match read_asset(ctx.app_handle(), request.uri().path()) {
        Some((content_type, bytes)) => respond(StatusCode::OK, content_type, bytes),
        None => respond(StatusCode::NOT_FOUND, "text/plain", b"not found".to_vec()),
    }
}

/// O arquivo que o esquema serve para `request_path`, ou `None` (raiz ausente, caminho fora da raiz,
/// arquivo inexistente). Também é o que o host HTTP do E2E (`e2e_host.rs`) usa, já que o Chromium do
/// Playwright não conhece o esquema `mika-live2d`.
pub(crate) fn read_asset<R: Runtime>(app: &AppHandle<R>, request_path: &str) -> Option<(&'static str, Vec<u8>)> {
    let root = live2d_root(app).ok()?;
    let resolved = resolve_asset_path(&root, request_path).ok()?;
    let bytes = std::fs::read(&resolved).ok()?;
    Some((content_type(&resolved), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn only_the_app_origins_are_echoed_for_cors() {
        let echoed = |o: Option<&str>| with_cors(respond(StatusCode::OK, "text/plain", Vec::new()), o)
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap().to_owned());
        assert_eq!(echoed(Some("http://tauri.localhost")).as_deref(), Some("http://tauri.localhost"));
        assert_eq!(echoed(Some("tauri://localhost")).as_deref(), Some("tauri://localhost"));
        assert_eq!(echoed(Some("https://evil.example")), None);
        assert_eq!(echoed(None), None);
    }

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-live2d-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("haru")).unwrap();
        fs::write(dir.join("haru/haru_greeter_t05.model3.json"), b"{}").unwrap();
        dir
    }

    #[test]
    fn happy_path_resolves_inside_the_root() {
        let root = temp_root("happy");
        let resolved = resolve_asset_path(&root, "/haru/haru_greeter_t05.model3.json").unwrap();
        assert_eq!(resolved, root.canonicalize().unwrap().join("haru/haru_greeter_t05.model3.json"));
    }

    #[test]
    fn literal_dotdot_is_rejected() {
        let root = temp_root("dotdot");
        assert_eq!(resolve_asset_path(&root, "/../secret").unwrap_err(), PathError::Escaped);
    }

    /// O caso que a diretriz pede nomeado: `..` disfarçado em percent-encoding.
    #[test]
    fn percent_encoded_dotdot_is_rejected() {
        let root = temp_root("encoded-dotdot");
        assert_eq!(resolve_asset_path(&root, "/%2e%2e/%2e%2e/secret").unwrap_err(), PathError::Escaped);
    }

    /// No alvo real do produto (Windows), `Path` reconhece `C:` como `Component::Prefix` mesmo
    /// vindo de uma URI com barra normal — a mesma checagem "só `Normal` passa" barra tanto o
    /// caminho absoluto quanto o prefixo de drive sem precisar de um caso especial para cada um.
    #[test]
    fn absolute_path_and_drive_prefix_are_rejected() {
        let root = temp_root("absolute");
        assert_eq!(resolve_asset_path(&root, "/C:/Windows/system.ini").unwrap_err(), PathError::Escaped);
    }

    /// "Se viável": criar symlink de arquivo no Windows exige privilégio (Developer Mode ou
    /// admin) que o ambiente de CI/dev pode não ter — pula em vez de falhar por permissão, que
    /// não é o que este teste audita.
    #[test]
    fn symlink_escaping_the_root_is_rejected_when_the_environment_allows_creating_one() {
        let root = temp_root("symlink");
        let outside = std::env::temp_dir().join(format!("mika-live2d-outside-{}", std::process::id()));
        fs::write(&outside, b"secret").unwrap();
        let link = root.join("escape.json");

        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_file(&outside, &link).is_ok();
        #[cfg(not(windows))]
        let created = std::os::unix::fs::symlink(&outside, &link).is_ok();

        if !created {
            eprintln!("symlink não pôde ser criado neste ambiente (sem privilégio) — teste pulado");
            let _ = fs::remove_file(&outside);
            return;
        }

        assert_eq!(resolve_asset_path(&root, "/escape.json").unwrap_err(), PathError::Escaped);
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn nul_byte_is_rejected() {
        let root = temp_root("nul");
        assert_eq!(resolve_asset_path(&root, "/haru/%00").unwrap_err(), PathError::Escaped);
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        let root = temp_root("missing");
        assert_eq!(resolve_asset_path(&root, "/haru/does-not-exist.json").unwrap_err(), PathError::Escaped);
    }

    #[test]
    fn missing_root_is_an_error_not_a_panic() {
        let root = std::env::temp_dir().join(format!("mika-live2d-test-no-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(resolve_asset_path(&root, "/haru/haru_greeter_t05.model3.json").unwrap_err(), PathError::Escaped);
    }

    #[test]
    fn content_type_covers_the_live2d_family_by_final_extension() {
        assert_eq!(content_type(Path::new("x/a.model3.json")), "application/json");
        assert_eq!(content_type(Path::new("x/a.motion3.json")), "application/json");
        assert_eq!(content_type(Path::new("x/a.exp3.json")), "application/json");
        assert_eq!(content_type(Path::new("x/a.physics3.json")), "application/json");
        assert_eq!(content_type(Path::new("x/a.moc3")), "application/octet-stream");
        assert_eq!(content_type(Path::new("x/preview.png")), "image/png");
    }
}
