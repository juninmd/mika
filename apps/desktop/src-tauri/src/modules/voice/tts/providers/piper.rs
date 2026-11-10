//! Piper: TTS neural rodando 100% na máquina via subprocesso — porte de
//! `apps/desktop/src/main/services/voice/tts/providers/piper.provider.ts`
//! (`resolvePiperBinary`/`ensurePiperVoice`/`generatePiperAudio`, commit `99ddf7c6` antes da
//! remoção do Electron). Era um dos dois cortes nomeados de `dispatch.rs`; sai de lá nesta rodada.
//!
//! **Contrato de linha de comando, fiel ao TS.** `--model <onnx> --output_file <wav>
//! [--length_scale <n>]`, texto pelo STDIN (não por argumento — a fala da Mika tem aspas/acentos,
//! e argumento de linha de comando é onde isso vira injeção ou truncamento), áudio por ARQUIVO
//! (não STDOUT — o Piper mistura log em stderr e dados em stdout, e WAV lido de um stream
//! compartilhado com log é a origem clássica de arquivo corrompido). `NO shell`: `Command` sem
//! `.arg("/C")`/`sh -c`, argumentos como vetor — nunca concatenados numa string.
//!
//! **O binário NÃO é baixado.** Mesma decisão do TS: executar um executável sem o usuário pedir é
//! escolha dele, não da Mika. Só o `.onnx`/`.onnx.json` da voz (dado, não código) é baixado, atrás
//! do MESMO chokepoint de privacidade que `providers/supertonic/assets.rs` já usa
//! (`assert_network_allowed`, modo offline bloqueia). Só voz do [`catalog`] baixa (revisão fixa do
//! repositório + SHA-256 por arquivo + licença do dataset conferida). O Supertonic cai para uma
//! destas vozes quando não fala o idioma da interface (`dispatch.rs::generate_local`).
//!
//! **Cancelamento por polling, não `race_cancel`.** `providers/remote/http.rs::race_cancel` corre
//! contra um `Future` (a requisição HTTP é só isso); um `std::process::Child` rodando dentro de
//! `spawn_blocking` não é `.await`ável por fora — mesma restrição que `providers/supertonic/
//! model.rs::denoise` já documenta para `ort::Session::run`. A checagem aqui é o mesmo desenho:
//! poll de `cancel.is_cancelled()` a cada rodada, `child.kill()` quando dispara.

pub(crate) mod catalog;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_network_allowed;
use crate::modules::settings::tf;
use crate::modules::voice::tts::abort_registry::CancelHandle;

/// Onde o Piper costuma estar quando o usuário o instalou manualmente. Ordem = preferência.
const BINARY_CANDIDATES: [&str; 2] = ["piper", "piper.exe"];

const SYNTHESIS_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Mensagem de erro no idioma da interface (`voice.tts.piper.*` no dicionário): chega à UI como o erro da fala.
fn msg(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    tf(key, locale, vars)
}

pub struct PiperOptions {
    pub voice: String,
    pub binary_path: String,
    pub rate: f32,
    pub cache_dir: PathBuf,
}

struct VoiceRef {
    key: String,
    path: String,
}

/// `pt_BR-faber-medium` -> `pt/pt_BR/faber/medium/pt_BR-faber-medium.onnx`. `None` para chave
/// malformada em vez de um caminho inventado — mesma escolha do TS: um 404 depois de baixar dezenas
/// de MB é um diagnóstico bem pior do que uma recusa imediata.
fn parse_voice_key(key: &str) -> Option<VoiceRef> {
    let trimmed = key.trim();
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let locale = parts[0];
    let quality = *parts.last()?;
    let speaker = parts[1..parts.len() - 1].join("-");
    if locale.is_empty() || speaker.is_empty() || quality.is_empty() {
        return None;
    }
    let language = locale.split('_').next()?;
    if language.is_empty() || !language.chars().all(|c| c.is_ascii_lowercase()) || !(2..=3).contains(&language.len()) {
        return None;
    }
    Some(VoiceRef { key: trimmed.to_string(), path: format!("{language}/{locale}/{speaker}/{quality}/{trimmed}.onnx") })
}

/// `rate` do resto do app (1.0 normal, 1.5 mais rápido) para `length_scale` do Piper, que anda no
/// sentido OPOSTO (maior = mais devagar). Teto/piso porque valores extremos produzem áudio
/// ininteligível em vez de rápido/lento.
fn rate_to_length_scale(rate: f32) -> f32 {
    if !rate.is_finite() || rate <= 0.0 {
        return 1.0;
    }
    (1.0 / rate).clamp(0.5, 2.0)
}

fn spawn_error_is_missing_binary(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}

/// A pergunta é "este executável existe?", não "ele sai com código 0" — versões do Piper divergem
/// no status de saída de `--help`. Só falha de spawn (binário ausente) significa ausência de
/// verdade, mesma regra do TS.
fn binary_exists(candidate: &str) -> bool {
    match Command::new(candidate).arg("--help").stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(_) => true,
        Err(err) => !spawn_error_is_missing_binary(&err),
    }
}

/// Config explícita primeiro (checagem de arquivo, não de execução — igual `existsSync` do TS);
/// depois o PATH. `Err` nomeado (não silêncio) para o chamador virar mensagem ao usuário.
fn resolve_binary(configured: &str, locale: &str) -> Result<String, String> {
    let configured = configured.trim();
    if !configured.is_empty() {
        return if Path::new(configured).exists() {
            Ok(configured.to_string())
        } else {
            Err(msg(locale, "voice.tts.piper.configuredPathMissing", &[("path", configured)]))
        };
    }
    for candidate in BINARY_CANDIDATES {
        if binary_exists(candidate) {
            return Ok(candidate.to_string());
        }
    }
    Err(msg(locale, "voice.tts.piper.binaryNotFound", &[]))
}

/// Baixa (se ausente) `.onnx`+`.onnx.json` da voz no cache local — mesmo chokepoint de privacidade
/// e mesmo padrão `.part` + rename atômico de `providers/supertonic/assets.rs::download_to_file`
/// ([`commit_download`]: download interrompido não pode deixar um cache que parece pronto). Só voz
/// do [`catalog`] baixa, e só com o SHA-256 batendo: bytes que não são os medidos nunca viram cache.
async fn ensure_voice(cache_dir: &Path, voice_key: &str, store: &ConfigStore) -> Result<PathBuf, String> {
    let locale = app_locale(store);
    let voice = parse_voice_key(voice_key).ok_or_else(|| msg(&locale, "voice.tts.piper.invalidVoiceKey", &[("voice", voice_key)]))?;

    // Cache ANTES do catálogo: o catálogo decide o que BAIXA, não o que já está no disco (uma voz que
    // saiu do catálogo depois de baixada continua falando).
    let model_path = cache_dir.join(format!("{}.onnx", voice.key));
    let config_path = cache_dir.join(format!("{}.onnx.json", voice.key));
    if model_path.is_file() && config_path.is_file() {
        return Ok(model_path);
    }
    let pinned = catalog::voice(&voice.key).ok_or_else(|| msg(&locale, "voice.tts.piper.voiceNotInCatalog", &[("voice", voice_key)]))?;

    tokio::fs::create_dir_all(cache_dir)
        .await
        .map_err(|e| msg(&locale, "voice.tts.piper.writeFailed", &[("path", &cache_dir.display().to_string()), ("detail", &e.to_string())]))?;
    let base = format!("https://huggingface.co/rhasspy/piper-voices/resolve/{}", catalog::REVISION);
    for (url, target, sha256) in [
        (format!("{base}/{}", voice.path), &model_path, pinned.onnx_sha256),
        (format!("{base}/{}.json", voice.path), &config_path, pinned.json_sha256),
    ] {
        if let Err(err) = download_voice_file(store, &url, target, sha256, &locale).await {
            let _ = tokio::fs::remove_file(&model_path).await;
            let _ = tokio::fs::remove_file(&config_path).await;
            return Err(err);
        }
    }
    Ok(model_path)
}

async fn download_voice_file(store: &ConfigStore, url: &str, target: &Path, sha256: &str, locale: &str) -> Result<(), String> {
    assert_network_allowed(store, url, "piper-tts")?;
    let failed = |detail: String| msg(locale, "voice.tts.piper.downloadFailed", &[("url", url), ("detail", &detail)]);
    let response = reqwest::Client::builder().timeout(DOWNLOAD_TIMEOUT).build().map_err(|e| failed(e.to_string()))?.get(url).send().await;
    let response = response.map_err(|e| failed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(failed(format!("HTTP {}", response.status().as_u16())));
    }
    let bytes = response.bytes().await.map_err(|e| failed(e.to_string()))?;
    commit_download(target, &bytes, sha256, locale).await
}

/// `.part` ÚNICO por download (processo + contador): dois `ensure_voice` simultâneos da mesma voz
/// escreviam no mesmo `.part`, e o `rename` de um levava o arquivo do outro.
fn part_path(target: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{}.part", std::process::id(), COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    target.with_file_name(name)
}

/// Só grava o que tem o SHA-256 medido no catálogo; qualquer outra coisa (CDN comprometida, ref
/// trocada, corpo truncado) é descartada antes de tocar o disco.
async fn commit_download(target: &Path, bytes: &[u8], expected_sha256: &str, locale: &str) -> Result<(), String> {
    let file_name = target.file_name().unwrap_or_default().to_string_lossy();
    if hex::encode(Sha256::digest(bytes)) != expected_sha256 {
        return Err(msg(locale, "voice.tts.piper.digestMismatch", &[("file", &file_name)]));
    }
    let part = part_path(target);
    let write_failed = |e: std::io::Error| msg(locale, "voice.tts.piper.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())]);
    if let Err(e) = tokio::fs::write(&part, bytes).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(write_failed(e));
    }
    tokio::fs::rename(&part, target).await.map_err(write_failed)
}

fn synthesis_output_path() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("mika-piper-{}-{unique}.wav", std::process::id()))
}

fn build_args(model_path: &Path, output_path: &Path, length_scale: f32) -> Vec<String> {
    let mut args = vec!["--model".to_string(), model_path.display().to_string(), "--output_file".to_string(), output_path.display().to_string()];
    if (length_scale - 1.0).abs() > f32::EPSILON {
        args.push("--length_scale".to_string());
        args.push(length_scale.to_string());
    }
    args
}

/// Roda o binário síncrono dentro do chamador de `spawn_blocking`, cancelável por polling (ver o
/// cabeçalho do arquivo) e sob timeout — os dois motivos por que isto não é um `.output()` cru.
fn run_synthesis(binary: &str, args: &[String], text: &str, cancel: &CancelHandle, locale: &str) -> Result<(), String> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| msg(locale, "voice.tts.piper.spawnFailed", &[("detail", &e.to_string())]))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }

    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait(); // colhe o processo: sem isso fica zumbi até o app fechar
            return Err("TTS cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(msg(locale, "voice.tts.piper.synthesisFailed", &[("status", &status.to_string())])),
            Ok(None) if started.elapsed() > SYNTHESIS_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(msg(locale, "voice.tts.piper.synthesisTimeout", &[]));
            }
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(e) => return Err(msg(locale, "voice.tts.piper.waitFailed", &[("detail", &e.to_string())])),
        }
    }
}

/// Porte de `generatePiperAudio`. `text` chega já sanitizado pelo chamador (`dispatch.rs`).
pub async fn generate(text: &str, opts: &PiperOptions, store: &ConfigStore, cancel: &CancelHandle) -> Result<Vec<u8>, String> {
    let locale = app_locale(store);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(msg(&locale, "voice.tts.piper.emptyText", &[]));
    }

    let binary = resolve_binary(&opts.binary_path, &locale)?;
    let model_path = ensure_voice(&opts.cache_dir, &opts.voice, store).await?;
    let length_scale = rate_to_length_scale(opts.rate);

    let output_path = synthesis_output_path();
    let args = build_args(&model_path, &output_path, length_scale);

    let text_owned = trimmed.to_string();
    let cancel_owned = cancel.clone();
    let locale_owned = locale.clone();
    let result = tokio::task::spawn_blocking(move || run_synthesis(&binary, &args, &text_owned, &cancel_owned, &locale_owned))
        .await
        .map_err(|e| msg(&locale, "voice.tts.piper.synthesisCancelled", &[("detail", &e.to_string())]))?;

    let audio = result.and_then(|()| {
        std::fs::read(&output_path).map_err(|e| msg(&locale, "voice.tts.piper.readFailed", &[("detail", &e.to_string())]))
    });
    let _ = std::fs::remove_file(&output_path);

    let audio = audio?;
    if audio.is_empty() {
        return Err(msg(&locale, "voice.tts.piper.emptyAudio", &[]));
    }
    Ok(audio)
}

#[cfg(test)]
mod tests;
