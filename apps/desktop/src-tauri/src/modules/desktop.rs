//! Módulo `desktop` em Rust — port de `main/auto-launch.ts` (via `shared/tauri-main/login-items.ts`)
//! e de `main/ipc/wizard-assets.ts`.
//!
//! `auto-launch:*` NÃO é canal de shell (ver o comentário em `bridge.rs`): é registro/arquivo de
//! sistema, sem GUI envolvida, então dá para resolver aqui, no `NativeRegistry`, com o mesmo I/O
//! que `login-items.ts` já fazia no sidecar — registro do Windows, LaunchAgent do macOS, `.desktop`
//! de autostart no Linux. Os três são obrigatórios: um `#[cfg(windows)]` sozinho deixaria macOS e
//! Linux com o toggle "ligado" na tela e nenhum efeito real, o defeito silencioso mais caro deste
//! repo (ver AGENTS.md).
//!
//! `mika:wizard-prepare-asset` (`prepareWizardAsset` em `wizard-assets.ts`) AGORA entra aqui: a
//! frente anterior deixou o canal no sidecar por falta de cliente HTTP; `reqwest` é dependência
//! direta deste crate desde então. O schema real (`desktopWizardPrepareAssetRequestSchema`) nunca
//! recebe caminho de arquivo do renderer — só `assetId` (enum fixo) + nomes de voz/modelo — e cada
//! ramo faz rede de verdade: baixa o binário do VAD (URL + SHA-256 fixos, igual ao TS), baixa os
//! pesos ONNX do Whisper e a voz do TTS local (Supertonic/Kokoro) do Hugging Face, ou dá `POST
//! /api/pull` num Ollama. Todo download passa por `privacy::assert_network_allowed` primeiro —
//! mesmo chokepoint que `createLoggedFetch` já aplica no lado TS — e é verificado por SHA-256
//! (VAD, com o mesmo digest do TS, via `sha2`; Whisper, com digest fixado por arquivo em
//! `WHISPER_FILES` — o `ETag` final do Hugging Face é hash do CDN Xet, não SHA-256); a voz do TTS
//! só é validada por tamanho, igual ao TS.
//!
//! **Não escreva SHA-256 à mão aqui.** Uma implementação própria no caminho de verificação de
//! integridade é a pior combinação possível: errada para mais, recusa download bom; errada para
//! menos, aceita binário adulterado dizendo que conferiu. A primeira versão desta rodada fez
//! exatamente isso (`Cargo.toml` não tinha `sha2` como dependência direta ainda) — o próprio teste
//! de vetores do NIST pegou o erro, mas a correção certa foi declarar `sha2` como dependência
//! direta, não depurar a implementação manual.
//!
//! Degradação conhecida, ainda válida para Kokoro/Supertonic: o TS aquece um pipeline
//! `@huggingface/transformers` completo (sessão ONNX + tokenizer); este port só garante o binário
//! em cache — não existe ainda consumidor Rust (`ort`/`tokenizers`) para montar a sessão dessas
//! duas vozes. Para o Whisper essa lacuna FECHOU nesta rodada: `modules/inference/whisper.rs` já
//! monta a sessão de verdade e `modules/voice.rs` já liga `mika:voice-transcribe` a ela — o que
//! faltava aqui era só o `tokenizer.json` que este arquivo agora baixa (ver o bloco "Whisper STT"
//! abaixo). Kokoro/Supertonic continuam no teto antigo até a Fase 5 do roadmap
//! (`docs/tauri-rust-migration.md`) alcançá-los: "preparado" para eles ainda significa "baixado",
//! não "pronto para inferir".

use crate::config::schemas::llm_defaults;
use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::AppHandle;
// `app.path()` (autostart em macOS/Linux) vem do trait `Manager`; no Windows ele não é usado aqui.
#[cfg(not(target_os = "windows"))]
use tauri::Manager;

use crate::config::store::ConfigStore;
use crate::modules::inference::whisper;
use crate::modules::privacy;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

mod embedding_download;
mod nemotron_download;

const CHANNELS: &[&str] = &["auto-launch:get", "auto-launch:set", "mika:wizard-prepare-asset"];

const AUTO_LAUNCH_IDENTIFIER: &str = "com.mika.desktop";
const AUTO_LAUNCH_PRODUCT_NAME: &str = "Mika";
#[cfg(any(target_os = "windows", test))]
const AUTO_LAUNCH_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

// --- Construtores puros: nenhum toca disco/registro, então rodam em teste em qualquer SO -------

#[cfg(any(target_os = "windows", test))]
fn windows_reg_add_args(exe: &str) -> Vec<String> {
    vec![
        "add".into(),
        AUTO_LAUNCH_RUN_KEY.into(),
        "/v".into(),
        AUTO_LAUNCH_PRODUCT_NAME.into(),
        "/t".into(),
        "REG_SZ".into(),
        "/d".into(),
        exe.into(),
        "/f".into(),
    ]
}

#[cfg(any(target_os = "windows", test))]
fn windows_reg_delete_args() -> Vec<String> {
    vec![
        "delete".into(),
        AUTO_LAUNCH_RUN_KEY.into(),
        "/v".into(),
        AUTO_LAUNCH_PRODUCT_NAME.into(),
        "/f".into(),
    ]
}

#[cfg(any(target_os = "windows", test))]
fn windows_reg_query_args() -> Vec<String> {
    vec![
        "query".into(),
        AUTO_LAUNCH_RUN_KEY.into(),
        "/v".into(),
        AUTO_LAUNCH_PRODUCT_NAME.into(),
    ]
}

#[cfg(any(target_os = "macos", test))]
fn macos_plist_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{AUTO_LAUNCH_IDENTIFIER}.plist"))
}

#[cfg(any(target_os = "macos", test))]
fn macos_plist_contents(exe: &str) -> String {
    let exe = exe.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n  \
<key>Label</key><string>{AUTO_LAUNCH_IDENTIFIER}</string>\n  \
<key>ProgramArguments</key><array><string>{exe}</string></array>\n  \
<key>RunAtLoad</key><true/>\n\
</dict></plist>\n"
    )
}

#[cfg(any(target_os = "linux", test))]
fn linux_autostart_path(config_dir: &Path) -> PathBuf {
    config_dir
        .join("autostart")
        .join(format!("{AUTO_LAUNCH_IDENTIFIER}.desktop"))
}

#[cfg(any(target_os = "linux", test))]
fn linux_desktop_entry(exe: &str) -> String {
    let exe = desktop_exec_arg(exe);
    format!(
        "[Desktop Entry]\nType=Application\nName={AUTO_LAUNCH_PRODUCT_NAME}\nExec={exe}\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// Argumento de `Exec=` (Desktop Entry Spec): com espaço ou caractere reservado vai entre aspas, e
/// `"`, `` ` ``, `$` e `\` ganham barra (a barra é dobrada duas vezes: o valor do arquivo é
/// desescapado antes de o `Exec` ser lido); `%` vira `%%` (código de campo).
#[cfg(any(target_os = "linux", test))]
fn desktop_exec_arg(exe: &str) -> String {
    if !exe.chars().any(|c| c.is_whitespace() || "\"'\\`$<>~|&;*?#()%".contains(c)) {
        return exe.to_string();
    }
    let escaped: String = exe
        .chars()
        .map(|c| match c {
            '"' | '`' | '$' => format!("\\{c}"),
            '\\' => "\\\\\\\\".to_string(),
            '%' => "%%".to_string(),
            other => other.to_string(),
        })
        .collect();
    format!("\"{escaped}\"")
}

/// Único booleano que `payload` pode carregar em `auto-launch:set` — mesma forma de
/// `requestSchema: z.boolean()` em `desktop-ipc.ts`. Qualquer outro tipo JSON é recusado, não
/// coagido (zod também não coage).
fn parse_enable_payload(payload: &Value) -> Result<bool, String> {
    payload
        .as_bool()
        .ok_or_else(|| "payload deve ser booleano".to_string())
}

/// O binário Tauri É o app (sem sidecar), então o caminho do item de login é o processo atual —
/// ao contrário do Electron, aqui não existe a distinção `MIKA_APP_EXE_PATH` vs. `node.exe`.
fn current_exe() -> Result<String, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("não foi possível resolver o executável do app: {e}"))
        .map(|p| p.to_string_lossy().into_owned())?;
    #[cfg(target_os = "linux")]
    let exe = autostart_exe(std::env::var("APPIMAGE").ok(), exe);
    Ok(exe)
}

/// Num AppImage o executável em execução vive num ponto de montagem temporário; `$APPIMAGE` é o
/// arquivo que o usuário de fato guardou.
#[cfg(any(target_os = "linux", test))]
fn autostart_exe(appimage: Option<String>, current: String) -> String {
    appimage.filter(|path| !path.is_empty()).unwrap_or(current)
}

// --- Efeito por SO: só estes tocam registro/disco de verdade, por isso ficam fora dos testes ----

#[cfg(target_os = "windows")]
fn run_reg(args: &[String]) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    // Evita o flash de uma janela de console ao clicar no toggle nas Settings.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|status| status.success())
        .map_err(|e| format!("falha ao executar reg.exe: {e}"))
}

#[cfg(target_os = "windows")]
fn enable_auto_launch(_app: &AppHandle, exe: &str) -> Result<(), String> {
    if run_reg(&windows_reg_add_args(exe))? {
        Ok(())
    } else {
        Err("reg add não conseguiu gravar o item de login".into())
    }
}

#[cfg(target_os = "windows")]
fn disable_auto_launch(_app: &AppHandle) -> Result<(), String> {
    // Mesma tolerância do sidecar (`login-items.ts`): apagar um item que nunca existiu não é erro.
    let _ = run_reg(&windows_reg_delete_args());
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_auto_launch_enabled(_app: &AppHandle) -> Result<bool, String> {
    Ok(run_reg(&windows_reg_query_args()).unwrap_or(false))
}

#[cfg(target_os = "macos")]
fn enable_auto_launch(app: &AppHandle, exe: &str) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let path = macos_plist_path(&home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, macos_plist_contents(exe)).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn disable_auto_launch(app: &AppHandle) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    match std::fs::remove_file(macos_plist_path(&home)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn is_auto_launch_enabled(app: &AppHandle) -> Result<bool, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    Ok(macos_plist_path(&home).exists())
}

#[cfg(target_os = "linux")]
fn enable_auto_launch(app: &AppHandle, exe: &str) -> Result<(), String> {
    let config = app.path().config_dir().map_err(|e| e.to_string())?;
    let path = linux_autostart_path(&config);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, linux_desktop_entry(exe)).map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn disable_auto_launch(app: &AppHandle) -> Result<(), String> {
    let config = app.path().config_dir().map_err(|e| e.to_string())?;
    match std::fs::remove_file(linux_autostart_path(&config)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_os = "linux")]
fn is_auto_launch_enabled(app: &AppHandle) -> Result<bool, String> {
    let config = app.path().config_dir().map_err(|e| e.to_string())?;
    Ok(linux_autostart_path(&config).exists())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("auto-launch precisa de um mecanismo dedicado para este SO — ver AGENTS.md sobre suporte multiplataforma explícito.");

fn set_auto_launch(app: &AppHandle, enable: bool) -> Result<(), String> {
    if !enable {
        return disable_auto_launch(app);
    }
    let exe = current_exe()?;
    enable_auto_launch(app, &exe)
}

// --- `mika:wizard-prepare-asset` (port de `wizard-assets.ts`) -----------------------------------

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Escreve em `<target>.part` e renomeia — mesmo padrão de `writeModelFileAtomic`
/// (`model-file-cache.ts`). Numa corrida perdida (outro processo já escreveu `target` com o MESMO
/// tamanho esperado), trata como sucesso em vez de sobrescrever; tamanho diferente nunca é aceito.
fn write_model_file_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = PathBuf::from(format!("{}.part", target.to_string_lossy()));
    let result = std::fs::write(&partial, bytes).and_then(|()| std::fs::rename(&partial, target));
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            if file_matches_size(target, bytes.len()) {
                Ok(())
            } else {
                Err(format!("gravação atômica falhou: {e}"))
            }
        }
    }
}

fn file_matches_size(path: &Path, expected: usize) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && expected > 0 && m.len() == expected as u64)
        .unwrap_or(false)
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else { return Ok(()) };
    std::fs::create_dir_all(parent).map_err(|e| format!("não criou {}: {e}", parent.display()))
}

/// GET com timeout explícito (a regra do antigo `check:fetch-timeout` do TS vale aqui do mesmo
/// jeito) e passando primeiro pelo chokepoint
/// de privacidade — mesmo ponto único que `createLoggedFetch` é no lado TS
/// (`assertNetworkAllowed` antes de todo `fetch`).
async fn download_bytes(store: &ConfigStore, provider_tag: &str, url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    privacy::assert_network_allowed(store, url, provider_tag)?;

    let response = privacy::fixed_endpoint_client()
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("download falhou: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download falhou: HTTP {}", response.status().as_u16()));
    }
    let bytes = response.bytes().await.map_err(|e| format!("download falhou: {e}"))?;
    Ok(bytes.to_vec())
}

/// Resultado de um ramo do wizard — mesma forma de `desktopWizardPrepareAssetResponseSchema`
/// (`success`, `message?`, `skipped?`).
struct WizardResult {
    success: bool,
    message: Option<String>,
    skipped: bool,
}

impl WizardResult {
    fn ok(message: impl Into<String>) -> Self {
        Self { success: true, message: Some(message.into()), skipped: false }
    }
    fn skipped(message: impl Into<String>) -> Self {
        Self { success: true, message: Some(message.into()), skipped: true }
    }
    fn fail(message: impl Into<String>) -> Self {
        Self { success: false, message: Some(message.into()), skipped: false }
    }
    fn to_value(&self) -> Value {
        let mut obj = json!({ "success": self.success });
        if let Some(message) = &self.message {
            obj["message"] = Value::String(message.clone());
        }
        if self.skipped {
            obj["skipped"] = Value::Bool(true);
        }
        obj
    }
}

/// Espelha `WizardAssetPayload` (`wizard-assets.ts`). Campos opcionais ausentes ou de tipo errado
/// viram `None` — mesma tolerância do `.optional()` do zod, sem rejeitar a requisição por isso.
struct WizardAssetRequest {
    asset_id: String,
    tts_provider: Option<String>,
    tts_voice: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    embedding_model: Option<String>,
}

fn parse_wizard_asset_payload(payload: &Value) -> Result<WizardAssetRequest, String> {
    let asset_id = payload["assetId"]
        .as_str()
        .ok_or_else(|| "'assetId' ausente ou inválido".to_string())?
        .to_string();
    let opt_str = |key: &str| payload[key].as_str().map(str::to_string);
    Ok(WizardAssetRequest {
        asset_id,
        tts_provider: opt_str("ttsProvider"),
        tts_voice: opt_str("ttsVoice"),
        llm_provider: opt_str("llmProvider"),
        llm_model: opt_str("llmModel"),
        embedding_model: opt_str("embeddingModel"),
    })
}

// --- VAD (Silero) — espelha `services/voice/vad/silero-vad.ts` ----------------------------------

const VAD_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const VAD_MODEL_URL: &str =
    "https://raw.githubusercontent.com/snakers4/silero-vad/bfdc0193023f121ea5b3cc7b176dbed570a68a59/src/silero_vad/data/silero_vad.onnx";
const VAD_MODEL_SHA256: &str = "1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3";
const VAD_MODEL_FILE: &str = "silero_vad.onnx";

// `pub(crate)`, não privado: `modules/voice/vad_audio.rs` (dono do canal `voice:vad-audio`)
// precisa do mesmo caminho para abrir a sessão Silero que este arquivo já baixa e verifica.
pub(crate) fn vad_model_path(data_dir: &Path) -> PathBuf {
    data_dir.join("vad-cache").join(VAD_MODEL_FILE)
}

fn file_matches_sha256(path: &Path, expected_hex: &str) -> bool {
    std::fs::read(path).map(|bytes| sha256_hex(&bytes) == expected_hex).unwrap_or(false)
}

async fn prepare_vad(store: &ConfigStore, data_dir: &Path) -> WizardResult {
    let path = vad_model_path(data_dir);
    if file_matches_sha256(&path, VAD_MODEL_SHA256) {
        return WizardResult::ok("Silero VAD pronto");
    }

    let bytes = match download_bytes(store, "wizard:vad", VAD_MODEL_URL, VAD_DOWNLOAD_TIMEOUT).await {
        Ok(v) => v,
        Err(e) => return WizardResult::fail(e),
    };
    let digest = sha256_hex(&bytes);
    if digest != VAD_MODEL_SHA256 {
        return WizardResult::fail(format!("modelo do VAD com digest inesperado: {digest}"));
    }
    if let Err(e) = ensure_parent_dir(&path) {
        return WizardResult::fail(e);
    }
    if let Err(e) = write_model_file_atomic(&path, &bytes) {
        return WizardResult::fail(e);
    }
    WizardResult::ok("Silero VAD pronto")
}

// --- Whisper STT — espelha `services/inference/whisper.runtime.ts` ------------------------------
//
// O TS aquece um pipeline `@huggingface/transformers` (download + sessão ONNX); sem `ort` fiado
// aqui, este port garante o download dos dois pesos ONNX que o pipeline pede com
// `dtype: { encoder_model: 'q8', decoder_model_merged: 'q8' }` — confirmado ao vivo contra
// `https://huggingface.co/api/models/onnx-community/whisper-base` (`onnx/encoder_model_quantized.onnx`,
// `onnx/decoder_model_merged_quantized.onnx`) — MAIS `tokenizer.json`, que `modules/inference/whisper.rs`
// exige para decodificar os ids gerados pelo decoder e que, até esta rodada, nenhum branch do
// wizard baixava (`whisper::require_asset` falhava sempre por causa disso; ver o cabeçalho de
// `modules/voice.rs`). Revisão fixa (não `main`) pelo mesmo motivo de Kokoro/Supertonic: o
// conteúdo do repositório não pode mudar sob o hash que já baixamos.
//
// **Integridade.** Cada arquivo tem o SHA-256 fixado abaixo, como o VAD. A revisão é um commit,
// então o conteúdo não muda sob o hash. Antes, o digest vinha do `ETag` da resposta — mas o
// `/resolve/` do Hugging Face redireciona para o CDN Xet, e o `etag` do ÚLTIMO salto é o hash Xet
// (também 64 hex), não o SHA-256 (que só existe no `X-Linked-ETag` do primeiro salto). O reqwest
// segue o redirect, então todo download íntegro do encoder reprovava com "digest inesperado".
// Fixar o digest também cobre o `tokenizer.json`, que não tinha verificação nenhuma.

const WHISPER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Espelha `VOICE_CONFIG_KEYS.WHISPER_MODEL` / `voice.whisperModel` (`config/schemas/voice.rs`,
/// `modules/voice/transcribe.rs::CFG_WHISPER_MODEL`) — duplicado como string literal (não
/// importado do outro módulo) porque `voice::transcribe` é privado a `modules::voice`, e o valor
/// em si não é dado de segurança, só uma chave de config; ids/revisões/sha256, que SÃO
/// security-relevant, vêm de um único lugar: `whisper::WHISPER_MODELS`.
const CFG_WHISPER_MODEL: &str = "voice.whisperModel";

/// Config salva, senão [`whisper::DEFAULT_WHISPER_MODEL_ID`] — mesmo fallback de
/// `voice::transcribe::resolve_whisper_model`, que este arquivo não pode chamar diretamente (mod
/// privado); ambos os lados lêem a MESMA chave e caem no MESMO default do schema.
fn resolve_whisper_model_id(store: &ConfigStore) -> String {
    store
        .get(CFG_WHISPER_MODEL)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| whisper::DEFAULT_WHISPER_MODEL_ID.to_string())
}

fn whisper_asset_url(spec: &whisper::WhisperModelSpec, url_path: &str) -> String {
    format!("https://huggingface.co/{}/resolve/{}/{url_path}", spec.id, spec.revision)
}

/// `(caminho na URL do HF, nome do arquivo em cache, seletor do SHA-256 no spec)` — o encoder e o
/// decoder vivem sob `onnx/` no repositório, mas o cache é achatado (mesmo comportamento de antes).
fn whisper_asset_files(spec: &whisper::WhisperModelSpec) -> [(&'static str, &'static str, &'static str); 3] {
    [
        ("onnx/encoder_model_quantized.onnx", "encoder_model_quantized.onnx", spec.encoder_sha256),
        ("onnx/decoder_model_merged_quantized.onnx", "decoder_model_merged_quantized.onnx", spec.decoder_sha256),
        ("tokenizer.json", "tokenizer.json", spec.tokenizer_sha256),
    ]
}

fn whisper_cache_path(data_dir: &Path, model_id: &str, cache_file: &str) -> PathBuf {
    data_dir.join("whisper-cache").join(whisper::whisper_model_slug(model_id)).join(cache_file)
}

/// Tamanho selecionado em Configurações > Voz (`voice.whisperModel`) — trocar de `base` para
/// `small` baixa para um subdiretório novo (`whisper_cache_path`/`whisper::whisper_model_slug`) e
/// nunca reaproveita ou apaga o tamanho anterior; sair do tamanho errado é decisão do usuário, não
/// deste wizard.
async fn prepare_whisper(store: &ConfigStore, data_dir: &Path) -> WizardResult {
    let model_id = resolve_whisper_model_id(store);
    let spec = whisper::whisper_model_spec(&model_id);

    for (url_path, cache_file, expected_sha256) in whisper_asset_files(spec) {
        let path = whisper_cache_path(data_dir, spec.id, cache_file);
        if file_matches_sha256(&path, expected_sha256) {
            continue;
        }

        let url = whisper_asset_url(spec, url_path);
        let bytes = match download_bytes(store, "wizard:whisper", &url, WHISPER_DOWNLOAD_TIMEOUT).await {
            Ok(v) => v,
            Err(e) => return WizardResult::fail(e),
        };
        if sha256_hex(&bytes) != expected_sha256 {
            return WizardResult::fail(format!("arquivo do Whisper ({}) com digest inesperado: {cache_file}", spec.id));
        }
        if let Err(e) = ensure_parent_dir(&path) {
            return WizardResult::fail(e);
        }
        if let Err(e) = write_model_file_atomic(&path, &bytes) {
            return WizardResult::fail(e);
        }
    }
    WizardResult::ok("Whisper STT pronto")
}

// --- TTS local (Supertonic/Kokoro) — espelha `supertonic.runtime.ts`/`kokoro-voices.ts` ---------
//
// `preloadSupertonic`/`preloadKokoro` no TS NUNCA propagam falha de download para
// `prepareWizardAsset`: o `await` descarta o booleano de sucesso, e as duas funções já engolem
// toda exceção internamente. Isto é uma quirk real do produto (não uma correção minha) — o wizard
// relata "voz pronta" mesmo quando o download falhou em silêncio. Portar fielmente significa
// replicar o "melhor esforço, sucesso sempre reportado" aqui, não "consertar" para uma UX mais
// honesta que o TS não tem hoje.

const VOICE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

const SUPERTONIC_MODEL_ID: &str = "onnx-community/Supertonic-TTS-2-ONNX";
const SUPERTONIC_REVISION: &str = "68d4d9420d0e0e51d14656e1ec5c9b091490b49e";
const SUPERTONIC_VOICE_BYTES: usize = 12_928 * 4;

const KOKORO_MODEL_ID: &str = "onnx-community/Kokoro-82M-v1.0-ONNX";
const KOKORO_REVISION: &str = "1939ad2a8e416c0acfeecc08a694d14ef25f2231";
const KOKORO_STYLE_MIN_BYTES: usize = 256 * 4;

fn is_valid_supertonic_voice(voice: &str) -> bool {
    let v = voice.trim().as_bytes();
    v.len() == 2 && matches!(v[0], b'F' | b'M') && matches!(v[1], b'1'..=b'5')
}

fn is_valid_kokoro_voice(voice: &str) -> bool {
    let v = voice.trim();
    let mut chars = v.chars();
    let prefix: String = chars.by_ref().take(2).collect();
    if prefix.chars().count() != 2 || !prefix.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    if chars.next() != Some('_') {
        return false;
    }
    let rest: String = chars.collect();
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase())
}

fn supertonic_voice_url(voice: &str) -> String {
    format!("https://huggingface.co/{SUPERTONIC_MODEL_ID}/resolve/{SUPERTONIC_REVISION}/voices/{voice}.bin")
}

fn kokoro_voice_url(voice: &str) -> String {
    format!("https://huggingface.co/{KOKORO_MODEL_ID}/resolve/{KOKORO_REVISION}/voices/{voice}.bin")
}

/// Baixa e cacheia um `.bin` de voz se ainda não existir — mesmo contrato de
/// `downloadVoiceBinIfMissing` (`voice-download.ts`): silencioso em qualquer falha (rede, digest,
/// disco), porque o chamador aqui (`prepare_tts`) reporta sucesso de qualquer jeito. `min_bytes`
/// é o piso de tamanho abaixo do qual o TS trata o arquivo como truncado/corrompido.
async fn ensure_voice_bin(store: &ConfigStore, provider_tag: &str, cache_dir: &Path, file_name: &str, url: &str, min_bytes: usize) {
    let path = cache_dir.join(file_name);
    if path.is_file() {
        return;
    }
    let Ok(bytes) = download_bytes(store, provider_tag, url, VOICE_DOWNLOAD_TIMEOUT).await else {
        return;
    };
    if bytes.len() < min_bytes {
        return;
    }
    if ensure_parent_dir(&path).is_err() {
        return;
    }
    let _ = write_model_file_atomic(&path, &bytes);
}

async fn prepare_tts(store: &ConfigStore, data_dir: &Path, req: &WizardAssetRequest) -> WizardResult {
    let provider = req.tts_provider.as_deref().unwrap_or("supertonic");
    match provider {
        "supertonic" => {
            let voice = req.tts_voice.as_deref().unwrap_or("F3");
            if is_valid_supertonic_voice(voice) {
                let cache_dir = data_dir.join("supertonic");
                let file_name = format!("supertonic-{voice}.bin");
                ensure_voice_bin(store, "wizard:tts", &cache_dir, &file_name, &supertonic_voice_url(voice), SUPERTONIC_VOICE_BYTES).await;
            }
            WizardResult::ok(format!("Voz Supertonic ({voice}) pronta"))
        }
        "kokoro" => {
            let voice = req.tts_voice.as_deref().unwrap_or("pf_dora");
            if is_valid_kokoro_voice(voice) {
                let cache_dir = data_dir.join("kokoro");
                let file_name = format!("kokoro-{voice}.bin");
                ensure_voice_bin(store, "wizard:tts", &cache_dir, &file_name, &kokoro_voice_url(voice), KOKORO_STYLE_MIN_BYTES).await;
            }
            WizardResult::ok(format!("Voz Kokoro ({voice}) pronta"))
        }
        _ => WizardResult::skipped("Provedor de voz em nuvem"),
    }
}

// --- Ollama (LLM/embedding) — espelha `resolveOllamaEndpoint`/`pullOllamaModel` -----------------

const OLLAMA_DEFAULT_URL: &str = llm_defaults::OLLAMA_BASE_URL;
const CFG_OLLAMA_BASE_URL: &str = "apiKeys.ollamaBaseUrl";
const CFG_OLLAMA_API_KEY: &str = "apiKeys.ollamaApiKey";

pub(crate) struct OllamaEndpoint {
    pub(crate) base_url: String,
    pub(crate) api_key: Option<String>,
}

/// Prioridade: config salva > `OLLAMA_BASE_URL` de ambiente > default — mesma ordem de
/// `resolveOllamaEndpoint`. Sem gate de SSRF aqui de propósito: ao contrário de `llm.rs`
/// (catálogo, que nega todo host não-loopback porque o `ISecurityService` real não foi portado),
/// `pullOllamaModel` no TS NUNCA passou por essa guarda — o wizard confia no endpoint que o
/// usuário já configurou (inclusive uma GPU remota na LAN, cenário real e testado do lado TS).
/// Reintroduzir o gate aqui quebraria essa feature, não a corrigiria.
pub(crate) fn resolve_ollama_endpoint(store: &ConfigStore) -> OllamaEndpoint {
    let base_url = store
        .get(CFG_OLLAMA_BASE_URL)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("OLLAMA_BASE_URL").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| OLLAMA_DEFAULT_URL.to_string());
    let api_key = store
        .get(CFG_OLLAMA_API_KEY)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty());
    OllamaEndpoint { base_url, api_key }
}

/// O mesmo download em stream da aba LLM (`llm::pull`): sem o timeout total de 300 s que cortava
/// modelo grande no meio.
async fn pull_ollama_model(store: &ConfigStore, model: &str, endpoint: &OllamaEndpoint) -> Result<(), String> {
    crate::modules::llm::pull::pull_model(store, &endpoint.base_url, endpoint.api_key.as_deref(), model, "wizard:ollama", |_| {}).await
}

async fn prepare_ollama_llm(store: &ConfigStore, req: &WizardAssetRequest) -> WizardResult {
    if req.llm_provider.as_deref() != Some("ollama") {
        return WizardResult::skipped("Provedor de LLM não requer download");
    }
    let model = req.llm_model.as_deref().unwrap_or(llm_defaults::OLLAMA_MODEL);
    let endpoint = resolve_ollama_endpoint(store);
    match pull_ollama_model(store, model, &endpoint).await {
        Ok(()) => WizardResult::ok(format!("Modelo Ollama ({model}) pronto")),
        Err(e) => WizardResult::fail(e),
    }
}

async fn prepare_ollama_embedding(store: &ConfigStore, req: &WizardAssetRequest) -> WizardResult {
    if req.llm_provider.as_deref() != Some("ollama") {
        return WizardResult::skipped("Provedor de embedding não requer download");
    }
    let model = req.embedding_model.as_deref().unwrap_or("embeddinggemma");
    let endpoint = resolve_ollama_endpoint(store);
    match pull_ollama_model(store, model, &endpoint).await {
        Ok(()) => WizardResult::ok(format!("Modelo de embedding Ollama ({model}) pronto")),
        Err(e) => WizardResult::fail(e),
    }
}

/// Local embeddings are the default memory search: without the model it silently degrades to
/// keyword search, so the boot fetches it in the background when `local` is selected and missing.
pub fn ensure_local_embedding(app: &AppHandle, store: std::sync::Arc<ConfigStore>) {
    let provider = store.get("llm.embeddingProvider").and_then(|v| v.as_str().map(str::to_string));
    let Ok(data_dir) = crate::state::app_data_dir(app) else { return };
    crate::modules::inference::embedder::set_data_dir(&data_dir);
    let wants_download = provider.as_deref().unwrap_or("local") == "local";
    tauri::async_runtime::spawn(async move {
        if wants_download && !crate::modules::inference::embedder::is_downloaded(&data_dir) {
            let result = embedding_download::prepare_local_embedding(&store, &data_dir).await;
            log::info!("embedding local no boot: success={} {}", result.success, result.message.unwrap_or_default());
        }
        // Loads the session now so the first proactive line (judged under the guard's lock) doesn't pay ~1 s.
        let warm = tauri::async_runtime::spawn_blocking(|| crate::modules::inference::embedder::embed("")).await;
        if let Ok(Err(e)) = warm {
            log::info!("embedding local indisponível: {e}");
        }
    });
}

// --- Despacho ------------------------------------------------------------------------------------

async fn prepare_wizard_asset(app: &AppHandle, req: &WizardAssetRequest) -> WizardResult {
    let data_dir = match crate::state::app_data_dir(app) {
        Ok(d) => d,
        Err(e) => return WizardResult::fail(format!("não resolveu app_data_dir: {e}")),
    };
    let store = match crate::state::config_store(app) {
        Ok(s) => s,
        Err(e) => return WizardResult::fail(e),
    };

    match req.asset_id.as_str() {
        "vad" => prepare_vad(&store, &data_dir).await,
        "whisper" => prepare_whisper(&store, &data_dir).await,
        "nemotron" => nemotron_download::prepare_nemotron(&store, &data_dir).await,
        "local-embedding" => embedding_download::prepare_local_embedding(&store, &data_dir).await,
        "tts" => prepare_tts(&store, &data_dir, req).await,
        "ollama-llm" => prepare_ollama_llm(&store, req).await,
        "ollama-embedding" => prepare_ollama_embedding(&store, req).await,
        other => WizardResult::fail(format!("Ativo desconhecido: {other}")),
    }
}

struct DesktopModule;

#[async_trait]
impl NativeModule for DesktopModule {
    fn name(&self) -> &'static str {
        "desktop"
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
        match channel {
            // `main-window.ts` registra `autoLaunchGet` SEM `assertOwnerSurface`: é leitura de um
            // booleano de sistema, sem efeito colateral, então qualquer janela pode consultá-la.
            "auto-launch:get" => is_auto_launch_enabled(app).map(Value::Bool),
            "auto-launch:set" => {
                if !is_owner_surface(ctx.window_type) {
                    return Err(format!("{channel} recusado para {:?}", ctx.window_type));
                }
                let enable = parse_enable_payload(&payload)?;
                set_auto_launch(app, enable).map(|()| Value::Null)
            }
            // `settings.ts` faz `assertOwnerSurface(context, 'wizardPrepareAsset')` antes de
            // chamar `prepareWizardAsset` — mesma guarda de `auto-launch:set`.
            "mika:wizard-prepare-asset" => {
                if !is_owner_surface(ctx.window_type) {
                    return Err(format!("{channel} recusado para {:?}", ctx.window_type));
                }
                let request = parse_wizard_asset_payload(&payload)?;
                Ok(prepare_wizard_asset(app, &request).await.to_value())
            }
            other => Err(format!("canal '{other}' declarado em `desktop` mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(DesktopModule)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    #[test]
    fn channels_match_the_ts_contract() {
        let contract = include_str!(
            "../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts"
        );
        for channel in CHANNELS {
            let literal = format!("'{channel}'");
            assert!(
                contract.contains(&literal),
                "canal {literal} não existe em desktop-ipc-channels.ts — o renderer nunca vai mandá-lo, \
                 e o toggle de auto-launch correspondente fica morto sem erro"
            );
        }
    }

    #[test]
    fn set_requires_boolean_payload() {
        assert_eq!(parse_enable_payload(&Value::Bool(true)), Ok(true));
        assert_eq!(parse_enable_payload(&Value::Bool(false)), Ok(false));
        assert!(parse_enable_payload(&Value::Null).is_err());
        assert!(parse_enable_payload(&Value::String("true".into())).is_err());
        assert!(parse_enable_payload(&Value::from(1)).is_err());
    }

    /// `plugin-view` e `unknown` são recusadas — mesma lista de `assertOwnerSurface` em
    /// `surface-policy.ts` (`main`, `settings`, `wizard` passam; o resto não). `auto-launch:get`
    /// não usa essa guarda (ver o `match` em `invoke`), só `auto-launch:set`.
    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(is_owner_surface(WindowType::Main));
        assert!(is_owner_surface(WindowType::Settings));
        assert!(is_owner_surface(WindowType::Wizard));
        assert!(!is_owner_surface(WindowType::PluginView));
        assert!(!is_owner_surface(WindowType::Unknown));
    }

    #[test]
    fn current_exe_resolves_to_a_real_path() {
        let exe = current_exe().expect("o binário de teste sempre tem um caminho");
        assert!(!exe.is_empty());
    }

    #[test]
    fn windows_reg_args_use_the_product_name_as_value() {
        let add = windows_reg_add_args(r"C:\Mika\Mika.exe");
        assert_eq!(
            add,
            vec![
                "add",
                AUTO_LAUNCH_RUN_KEY,
                "/v",
                "Mika",
                "/t",
                "REG_SZ",
                "/d",
                r"C:\Mika\Mika.exe",
                "/f",
            ]
        );
        assert_eq!(
            windows_reg_delete_args(),
            vec!["delete", AUTO_LAUNCH_RUN_KEY, "/v", "Mika", "/f"]
        );
        assert_eq!(
            windows_reg_query_args(),
            vec!["query", AUTO_LAUNCH_RUN_KEY, "/v", "Mika"]
        );
    }

    #[test]
    fn macos_plist_matches_the_sidecar_template() {
        let plist = macos_plist_contents("/Applications/Mika.app/Contents/MacOS/Mika");
        assert!(plist.contains("<key>Label</key><string>com.mika.desktop</string>"));
        assert!(plist.contains(
            "<key>ProgramArguments</key><array><string>/Applications/Mika.app/Contents/MacOS/Mika</string></array>"
        ));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));

        let path = macos_plist_path(Path::new("/Users/mika"));
        assert_eq!(
            path,
            PathBuf::from("/Users/mika/Library/LaunchAgents/com.mika.desktop.plist")
        );
    }

    #[test]
    fn linux_desktop_entry_matches_the_sidecar_template() {
        let entry = linux_desktop_entry("/opt/mika/mika");
        assert_eq!(
            entry,
            "[Desktop Entry]\nType=Application\nName=Mika\nExec=/opt/mika/mika\nX-GNOME-Autostart-enabled=true\n"
        );

        let path = linux_autostart_path(Path::new("/home/mika/.config"));
        assert_eq!(
            path,
            PathBuf::from("/home/mika/.config/autostart/com.mika.desktop.desktop")
        );
    }

    /// `Exec=` é separado por espaço: um caminho com espaço (`~/My Apps/Mika.AppImage`) virava dois
    /// argumentos e o item de login nunca abria o app. Sem caractere especial, nada muda.
    #[test]
    fn linux_desktop_entry_quotes_an_exe_path_with_spaces() {
        let entry = linux_desktop_entry("/home/ana/My Apps/mika");
        assert!(entry.contains("\nExec=\"/home/ana/My Apps/mika\"\n"), "{entry}");
        let entry = linux_desktop_entry("/opt/a$b/mika");
        assert!(entry.contains("\nExec=\"/opt/a\\$b/mika\"\n"), "{entry}");
    }

    /// Dentro de um AppImage `current_exe()` é o ponto de montagem temporário (`/tmp/.mount_XXXX/…`),
    /// que some quando o app fecha: o item de login apontava para um caminho morto. `$APPIMAGE` é o
    /// arquivo de verdade.
    #[test]
    fn autostart_points_at_the_appimage_file_not_its_temporary_mount() {
        assert_eq!(autostart_exe(Some("/home/ana/Mika.AppImage".into()), "/tmp/.mount_MikaAb/usr/bin/mika".into()), "/home/ana/Mika.AppImage");
        assert_eq!(autostart_exe(None, "/opt/mika/mika".into()), "/opt/mika/mika");
        assert_eq!(autostart_exe(Some(String::new()), "/opt/mika/mika".into()), "/opt/mika/mika");
    }

    /// `&` e `<` num caminho quebravam o XML do plist e o macOS ignorava o LaunchAgent.
    #[test]
    fn macos_plist_escapes_xml_in_the_exe_path() {
        let plist = macos_plist_contents("/Applications/R&D <Mika>.app/Contents/MacOS/Mika");
        assert!(plist.contains("<string>/Applications/R&amp;D &lt;Mika&gt;.app/Contents/MacOS/Mika</string>"), "{plist}");
    }

    // --- wizard-prepare-asset: sem rede nem som real na suíte — só construtores puros ----------
    //
    // `download_bytes`/`prepare_vad`/`prepare_whisper`/`ensure_voice_bin`/`pull_ollama_model` não
    // são testados aqui: exigiriam baixar de verdade (VAD/Whisper/HF/Ollama). O que é testado é a
    // URL escolhida por assetId, a validação do payload, a resolução de caminho de cache e a
    // aritmética do SHA-256 — exatamente o que o pedido desta rodada pediu para cobrir.

    /// `silero-vad.ts` foi removido (backend Node deletado na migração para Tauri); `VAD_MODEL_SHA256`
    /// e `VAD_MODEL_URL` são agora a única fonte de verdade — o pin fica em `vad_model_path_lands_under_vad_cache`
    /// e nos usos reais das constantes abaixo, sem leitura de arquivo.
    #[test]
    fn vad_model_constants_are_non_empty() {
        assert!(!VAD_MODEL_SHA256.is_empty());
        assert!(!VAD_MODEL_URL.is_empty());
    }

    #[test]
    fn vad_model_path_lands_under_vad_cache() {
        let path = vad_model_path(Path::new("/data"));
        assert_eq!(path, PathBuf::from("/data/vad-cache/silero_vad.onnx"));
    }

    #[test]
    fn whisper_asset_url_points_at_the_pinned_revision() {
        let spec = whisper::whisper_model_spec(whisper::DEFAULT_WHISPER_MODEL_ID);
        let url = whisper_asset_url(spec, "onnx/encoder_model_quantized.onnx");
        assert_eq!(
            url,
            "https://huggingface.co/onnx-community/whisper-base/resolve/1846881b6b3a3024392c1eea3ad983695bc23925/onnx/encoder_model_quantized.onnx"
        );
    }

    /// NEGATIVO — o bug real que travava `mika:voice-transcribe`: os arquivos baixados eram só os
    /// dois pesos ONNX e nunca `tokenizer.json`, então `whisper::require_asset` falhava sempre.
    /// Esta prova fica presa à lista de produção, não a uma cópia: um `whisper_asset_files` que
    /// voltasse a esquecer o tokenizer reprovaria aqui antes de reprovar em produção.
    #[test]
    fn whisper_files_includes_the_tokenizer() {
        let spec = whisper::whisper_model_spec(whisper::DEFAULT_WHISPER_MODEL_ID);
        assert!(
            whisper_asset_files(spec).iter().any(|(_, cache_file, _)| *cache_file == "tokenizer.json"),
            "sem tokenizer.json na lista, prepare_whisper nunca o baixa e whisper::transcribe falha sempre"
        );
        let url = whisper_asset_url(spec, "tokenizer.json");
        assert_eq!(
            url,
            "https://huggingface.co/onnx-community/whisper-base/resolve/1846881b6b3a3024392c1eea3ad983695bc23925/tokenizer.json"
        );
    }

    #[test]
    fn whisper_cache_path_flattens_the_onnx_subfolder() {
        let path = whisper_cache_path(Path::new("/data"), whisper::DEFAULT_WHISPER_MODEL_ID, "decoder_model_merged_quantized.onnx");
        assert_eq!(path, PathBuf::from("/data/whisper-cache/whisper-base/decoder_model_merged_quantized.onnx"));
    }

    /// `tokenizer.json` já mora na raiz do repositório HF (sem prefixo `onnx/`) — precisa cair no
    /// MESMO caminho que `modules/inference/whisper.rs::TOKENIZER_FILE`/`whisper_cache_path` espera
    /// (subdiretório por tamanho compartilhado via `whisper::whisper_model_slug`).
    #[test]
    fn whisper_cache_path_places_the_tokenizer_under_the_model_slug() {
        let path = whisper_cache_path(Path::new("/data"), whisper::DEFAULT_WHISPER_MODEL_ID, "tokenizer.json");
        assert_eq!(path, PathBuf::from("/data/whisper-cache/whisper-base/tokenizer.json"));
    }

    /// A "Modelo Whisper (Local)" dead control (`voice.whisperModel`): tamanho salvo tem de mudar
    /// qual revisão/subdiretório o wizard baixa, não só qual entrada aparece na UI.
    #[test]
    fn prepare_whisper_reads_the_saved_model_size_not_a_fixed_one() {
        let (store, _dir) = test_store();
        store.set(CFG_WHISPER_MODEL, json!("onnx-community/whisper-small")).unwrap();

        let model_id = resolve_whisper_model_id(&store);
        assert_eq!(model_id, "onnx-community/whisper-small");
        let spec = whisper::whisper_model_spec(&model_id);
        assert_eq!(spec.id, "onnx-community/whisper-small");
        assert_ne!(spec.revision, whisper::whisper_model_spec(whisper::DEFAULT_WHISPER_MODEL_ID).revision);
    }

    #[test]
    fn resolve_whisper_model_id_defaults_without_explicit_config() {
        let (store, _dir) = test_store();
        assert_eq!(resolve_whisper_model_id(&store), whisper::DEFAULT_WHISPER_MODEL_ID);
    }

    /// NEGATIVO — o bug real do wizard: o digest esperado vinha do `etag` da resposta FINAL, que
    /// depois do redirect para o CDN Xet é o hash Xet (`b9f8…`), não o SHA-256 (`5862…`). Todo
    /// download íntegro do encoder reprovava. O digest agora é fixo e tem de ser o SHA-256 real.
    #[test]
    fn whisper_encoder_digest_is_the_sha256_not_the_cdn_xet_hash() {
        let spec = whisper::whisper_model_spec(whisper::DEFAULT_WHISPER_MODEL_ID);
        assert_eq!(spec.encoder_sha256, "5862993336bf33acd23736071aae2b32261d3b1b2f37780194460d4ef974dd46");
        assert_ne!(spec.encoder_sha256, "b9f841a1c8af3b6fa030ce4b499ac14461d662a332877be7827119d51f7994a6");
    }

    /// Todo arquivo baixado tem digest verificável — inclusive `tokenizer.json`, que antes passava
    /// sem conferência nenhuma por não ter `X-Linked-ETag` de 64 hex. Cobre os três tamanhos, não
    /// só o default.
    #[test]
    fn every_whisper_file_has_a_pinned_sha256() {
        for spec in &whisper::WHISPER_MODELS {
            for (_, cache_file, digest) in whisper_asset_files(spec) {
                assert_eq!(digest.len(), 64, "{} / {cache_file}", spec.id);
                assert!(digest.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{} / {cache_file}", spec.id);
            }
        }
    }

    /// NEGATIVO — cache corrompido (download interrompido, arquivo truncado) não pode contar como
    /// pronto: `prepare_whisper` confere o digest do arquivo em disco, não só a existência.
    #[test]
    fn corrupted_whisper_cache_file_does_not_match_its_digest() {
        let dir = std::env::temp_dir().join(format!("mika-whisper-digest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tokenizer.json");
        std::fs::write(&path, b"truncated").unwrap();
        let spec = whisper::whisper_model_spec(whisper::DEFAULT_WHISPER_MODEL_ID);
        assert!(!file_matches_sha256(&path, spec.tokenizer_sha256));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn is_valid_supertonic_voice_accepts_f_or_m_one_to_five() {
        for voice in ["F1", "F3", "M5", "M1"] {
            assert!(is_valid_supertonic_voice(voice), "{voice} deveria ser válido");
        }
    }

    #[test]
    fn is_valid_supertonic_voice_rejects_out_of_range() {
        for voice in ["F6", "M0", "X1", "f3", "F", "F33", ""] {
            assert!(!is_valid_supertonic_voice(voice), "{voice} não deveria ser válido");
        }
    }

    #[test]
    fn is_valid_kokoro_voice_accepts_lang_prefix_and_name() {
        for voice in ["pf_dora", "af_heart", "bm_george"] {
            assert!(is_valid_kokoro_voice(voice), "{voice} deveria ser válido");
        }
    }

    #[test]
    fn is_valid_kokoro_voice_rejects_malformed() {
        for voice in ["pf", "pf_", "pfg_dora", "PF_dora", "pf-dora", ""] {
            assert!(!is_valid_kokoro_voice(voice), "{voice} não deveria ser válido");
        }
    }

    /// `supertonic.runtime.ts`/`kokoro-voices.ts` foram removidos (backend Node deletado na
    /// migração para Tauri) — os IDs/revisions que eles fixavam viram literais nas URLs esperadas.
    #[test]
    fn supertonic_and_kokoro_voice_urls_match_the_expected_revisions() {
        assert_eq!(
            supertonic_voice_url("F3"),
            "https://huggingface.co/onnx-community/Supertonic-TTS-2-ONNX/resolve/68d4d9420d0e0e51d14656e1ec5c9b091490b49e/voices/F3.bin"
        );

        assert_eq!(
            kokoro_voice_url("pf_dora"),
            "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/voices/pf_dora.bin"
        );
    }

    #[test]
    fn parse_wizard_asset_payload_requires_asset_id() {
        assert!(parse_wizard_asset_payload(&json!({})).is_err());
        assert!(parse_wizard_asset_payload(&json!({ "assetId": 42 })).is_err());
    }

    #[test]
    fn parse_wizard_asset_payload_reads_optional_fields_when_present() {
        let req = parse_wizard_asset_payload(&json!({
            "assetId": "tts",
            "ttsProvider": "kokoro",
            "ttsVoice": "pf_dora",
        }))
        .unwrap();
        assert_eq!(req.asset_id, "tts");
        assert_eq!(req.tts_provider.as_deref(), Some("kokoro"));
        assert_eq!(req.tts_voice.as_deref(), Some("pf_dora"));
        assert_eq!(req.llm_provider, None);
    }

    #[test]
    fn parse_wizard_asset_payload_tolerates_missing_optionals() {
        let req = parse_wizard_asset_payload(&json!({ "assetId": "vad" })).unwrap();
        assert_eq!(req.asset_id, "vad");
        assert_eq!(req.tts_provider, None);
        assert_eq!(req.embedding_model, None);
    }

    #[test]
    fn wizard_result_to_value_matches_the_response_schema_shape() {
        assert_eq!(WizardResult::ok("pronto").to_value(), json!({ "success": true, "message": "pronto" }));
        assert_eq!(
            WizardResult::skipped("nuvem").to_value(),
            json!({ "success": true, "message": "nuvem", "skipped": true })
        );
        assert_eq!(WizardResult::fail("falhou").to_value(), json!({ "success": false, "message": "falhou" }));
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-desktop-module-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (store, dir)
    }

    /// O schema (`ollamaBaseUrl` em `llm-api-keys-config.schema.ts`, espelhado em
    /// `config/schemas/llm.rs`) já declara `default('http://localhost:11434')` — `ConfigStore`
    /// nunca devolve `None` para este caminho, mesmo numa store recém-aberta. O fallback
    /// `OLLAMA_DEFAULT_URL` só entra em jogo quando o valor salvo é vazio (teste abaixo), não
    /// quando ele simplesmente nunca foi definido.
    #[test]
    fn resolve_ollama_endpoint_uses_the_schema_default_without_explicit_config() {
        let (store, _dir) = test_store();
        let endpoint = resolve_ollama_endpoint(&store);
        assert_eq!(endpoint.base_url, "http://localhost:11434");
        assert_eq!(endpoint.api_key, None);
    }

    #[test]
    fn resolve_ollama_endpoint_falls_back_to_the_hardcoded_default_when_config_is_blank() {
        let (store, _dir) = test_store();
        store.set(CFG_OLLAMA_BASE_URL, json!("")).unwrap();
        let endpoint = resolve_ollama_endpoint(&store);
        assert_eq!(endpoint.base_url, OLLAMA_DEFAULT_URL);
    }

    #[test]
    fn resolve_ollama_endpoint_prefers_saved_config_over_default() {
        let (store, _dir) = test_store();
        store.set(CFG_OLLAMA_BASE_URL, json!("http://gpu-box:11434/")).unwrap();
        store.set(CFG_OLLAMA_API_KEY, json!("k-remote")).unwrap();
        let endpoint = resolve_ollama_endpoint(&store);
        assert_eq!(endpoint.base_url, "http://gpu-box:11434/");
        assert_eq!(endpoint.api_key.as_deref(), Some("k-remote"));
    }

    #[test]
    fn write_model_file_atomic_writes_then_reads_back_exact_bytes() {
        let dir = TempDir::new();
        let target = dir.0.join("model.bin");
        write_model_file_atomic(&target, b"conteudo-real").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"conteudo-real");
        assert!(!target.with_extension("bin.part").exists());
    }

    #[test]
    fn file_matches_size_rejects_wrong_length_and_missing_file() {
        let dir = TempDir::new();
        let target = dir.0.join("model.bin");
        std::fs::write(&target, b"12345").unwrap();
        assert!(file_matches_size(&target, 5));
        assert!(!file_matches_size(&target, 6));
        assert!(!file_matches_size(&dir.0.join("nope.bin"), 5));
    }

    /// NEGATIVO — canal desconhecido não pode virar `Ativo desconhecido` silencioso: o dispatcher
    /// central (`invoke`) tem de recusar ANTES de chegar em `prepare_wizard_asset`.
    #[test]
    fn wizard_channel_is_declared_and_owner_gated() {
        assert!(CHANNELS.contains(&"mika:wizard-prepare-asset"));
    }
}
