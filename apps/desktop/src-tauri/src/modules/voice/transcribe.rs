//! `mika:voice-transcribe` — porte de `runSttProvider`
//! (`apps/desktop/src/main/services/voice/stt/stt-provider-runner.ts`) sobre
//! `inference::whisper::transcribe` (Whisper LOCAL) e [`super::stt_cloud`] (Groq/OpenAI, nuvem).
//! Ver o cabeçalho de `modules/voice.rs` (agora `voice/mod.rs`) para o histórico do bloqueio que
//! isto destrava.
//!
//! **O que este porte cobre, e o que fica de fora — de propósito, não por esquecimento:**
//! - `electron`/`local-whisper` (`voice.asrProvider`) rodam Whisper local aqui embaixo.
//!   `groq-whisper`/`openai-whisper` roteiam para [`super::stt_cloud::transcribe`] — mesmo
//!   dispatch, sem caminho paralelo. `browser` já era renderer-only no lado TS (`runSttProvider`
//!   lança para esse valor) e continua sem porte: [`unsupported_provider_response`] cobre esse e
//!   qualquer provider desconhecido.
//! - Áudio chega sempre como PCM16 mono cru em base64 (`audio/pcm;rate=16000`,
//!   `speech-transcribe.ts::PCM_MIME_TYPE`) para o caminho normal de fala; qualquer outro
//!   `mimeType` (webm/ogg do `MediaRecorder`, usado pela tela de homologação) já era rejeitado
//!   pelo Whisper local no lado TS também (`stt-provider-runner.ts` lança a mesma mensagem) —
//!   não é uma regressão nova, só reproduzida aqui. O caminho de nuvem NÃO rejeita: espelha
//!   `toUploadable`, que sobe qualquer mimeType não-PCM cru (ver `stt_cloud.rs`).
//! - **Degradação conhecida, aceita conscientemente:** o TS roda `UtteranceSpeechGate.check`
//!   antes do Whisper para descartar áudio sem fala (`ipc.ts`, chokepoint de VAD). Este port não
//!   replica esse pré-filtro — portá-lo exigiria orquestrar `inference::vad::SileroVad` janela a
//!   janela sobre um buffer de tamanho variável, fora do que esta rodada pediu (ligar o canal ao
//!   Whisper já pronto). Efeito prático: áudio sem fala roda o pipeline Whisper inteiro em vez de
//!   ser descartado cedo — mais lento, e o Whisper pode alucinar texto de ruído em vez de devolver
//!   vazio. Não é risco de segurança nem de contrato de dados, só de qualidade/custo; destrava
//!   quando alguém tiver este módulo no escopo para portar o gate.
//!   Hoje o pré-filtro é o veredito do VAD em Rust ([`neural_vad_rejects`]): motor `silero` descarta
//!   trecho sem voz; `voice.vadIgnoreTyping` descarta, em qualquer motor, o trecho em que o VAD viu
//!   tecla e nenhuma voz (`speech_gate.rs`).

use std::path::PathBuf;

use base64::Engine as _;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::modules::inference::{nemotron, whisper};

/// Espelha `VOICE_CONFIG_KEYS.ASR_LANGUAGE`/o campo `asrProvider` (`constants.ts`,
/// `voice-stt-config.schema.ts`) — os dois únicos caminhos de config que este canal lê.
const CFG_ASR_PROVIDER: &str = "voice.asrProvider";
const CFG_ASR_LANGUAGE: &str = "voice.asrLanguage";
/// Espelha `VOICE_CONFIG_KEYS.WHISPER_MODEL` (`constants.ts`) — o tamanho do Whisper local
/// escolhido em Configurações > Voz (`voice-stt-config.schema.ts::whisperModel`).
const CFG_WHISPER_MODEL: &str = "voice.whisperModel";

/// Mesmo default do schema (`config/schemas/voice.rs`): Nemotron, que não alucina texto sobre ruído.
const DEFAULT_ASR_PROVIDER: &str = "nemotron";

/// Espelha `WHISPER_SAMPLE_RATE` (`constants.ts`) — o rate que o renderer sempre manda hoje
/// (`speech-transcribe.ts::PCM_MIME_TYPE`) e o único que `inference::whisper::transcribe` aceita.
const DEFAULT_PCM_SAMPLE_RATE: u32 = 16_000;
/// Motor que decide se a fala recortada pelo renderer vai para o STT (`energy` | `silero`).
const CFG_VAD_ENGINE: &str = "voice.vadEngine";
/// Folga além da duração da fala: decodificação no renderer + latência do feed do VAD.
const NEURAL_VAD_SLACK_MS: u64 = 1_000;
/// Casa `IGNORABLE_TRANSCRIBE_ERROR` (`speech-transcribe.ts`): descarte silencioso, não erro na tela.
const NEURAL_VAD_NO_SPEECH: &str = "sem fala: o VAD neural (Silero) não ouviu voz neste trecho";

/// Teto de `audioBase64`: ~5 minutos de PCM16 mono a 16 kHz (`5*60*16000*2` bytes brutos, ~1.34x em
/// base64) arredondado para cima. Nenhuma UI deste app grava utterance tão longa — `voice:mic-state`
/// já corta a captura bem antes disso. Sem este teto, `mika:voice-transcribe` é canal isolado (ver
/// AGENTS.md §"Isolamento de processo") mas sem limite superior de tamanho: um payload de centenas
/// de MB passaria pelo `min(1)` do schema TS e chegaria inteiro ao Whisper, tudo em memória.
const MAX_AUDIO_BASE64_CHARS: usize = 16_000_000;

/// Payload de `mika:voice-transcribe`, espelhando `desktopTranscribeRequestSchema`
/// (`audioBase64` obrigatório e não-vazio; `mimeType`/`language` opcionais).
///
/// `pub(crate)`: `voice::voice_tool` (ação `transcribe` da tool `voice`, TS `tools/voice.ts`)
/// monta este MESMO tipo para reusar [`transcribe_pcm`] em vez de duplicar a resolução de
/// provedor/mimeType/sample-rate.
pub(crate) struct TranscribeRequest {
    pub(crate) audio_base64: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) language: Option<String>,
}

/// `audioBase64` ausente/vazio é violação de contrato — o schema Zod do renderer já garante
/// `min(1)` antes do payload sair de lá, então chegar aqui sem ele é bug, não entrada de usuário.
/// Por isso vira `Err` (o `invoke` rejeita), mesmo padrão de `extract_enabled` em `voice/mod.rs`.
fn parse_payload(payload: &Value) -> Result<TranscribeRequest, String> {
    let audio_base64 = payload["audioBase64"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "'audioBase64' ausente ou vazio".to_string())?
        .to_string();
    if audio_base64.len() > MAX_AUDIO_BASE64_CHARS {
        return Err(format!(
            "'audioBase64' excede o teto de {MAX_AUDIO_BASE64_CHARS} caracteres (~5min de PCM16 mono 16kHz)"
        ));
    }
    Ok(TranscribeRequest {
        audio_base64,
        mime_type: payload["mimeType"].as_str().map(str::to_string),
        language: payload["language"].as_str().map(str::to_string),
    })
}

/// `true` = descartar: o VAD rodou sem ouvir voz (Silero + `speech_gate.rs`) E o motor é `silero`, ou
/// é `energy` com `voice.vadIgnoreTyping` e o trecho teve tecla. Sem veredito (VAD mudo), segue.
pub(super) fn neural_vad_rejects(engine: &str, ignore_typing: bool, verdict: Option<bool>, keystrokes: bool) -> bool {
    verdict == Some(false) && (engine == "silero" || (ignore_typing && keystrokes))
}

/// Duração do PCM16 mono 16 kHz vinda em base64, mais [`NEURAL_VAD_SLACK_MS`].
fn pcm_window_ms(base64_len: usize) -> u64 {
    let bytes = (base64_len as u64) * 3 / 4;
    bytes / 2 * 1_000 / u64::from(DEFAULT_PCM_SAMPLE_RATE) + NEURAL_VAD_SLACK_MS
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Config salva, senão [`DEFAULT_ASR_PROVIDER`] (o mesmo default do schema).
pub(crate) fn resolve_provider(store: &ConfigStore) -> String {
    store
        .get(CFG_ASR_PROVIDER)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_ASR_PROVIDER.to_string())
}

/// Config salva, senão [`whisper::DEFAULT_WHISPER_MODEL_ID`] (o mesmo default do schema,
/// `config/schemas/voice.rs`). Lido aqui (não dentro de `whisper::transcribe`) porque só este
/// módulo tem `ConfigStore` — `whisper.rs` recebe o id já resolvido, mesmo padrão de `provider`.
pub(crate) fn resolve_whisper_model(store: &ConfigStore) -> String {
    store
        .get(CFG_WHISPER_MODEL)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| whisper::DEFAULT_WHISPER_MODEL_ID.to_string())
}

/// Os motores que rodam no próprio app (Whisper local e Nemotron). `groq-whisper`/
/// `openai-whisper` são checados ANTES desta função em [`transcribe_pcm`] (via
/// `stt_cloud::CloudSttProvider::from_provider_id`); só chegam aqui `browser` e qualquer valor
/// desconhecido, que [`unsupported_provider_response`] cobre.
fn is_local_provider(provider: &str) -> bool {
    matches!(provider, "electron" | "local-whisper" | "nemotron")
}

/// PCM16 LE → [`super::stt_cleanup::trim_silence`] → PCM16 LE. Outro rate passa intacto: quem
/// recusa, alto, é o motor (contrato PCM16 mono 16 kHz).
fn cleaned_pcm(pcm: Vec<u8>, sample_rate: u32) -> Vec<u8> {
    if sample_rate != DEFAULT_PCM_SAMPLE_RATE || pcm.len() % 2 != 0 {
        return pcm;
    }
    let mut samples: Vec<i16> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    let removed = super::stt_cleanup::trim_silence(&mut samples);
    log::debug!("[STT] {} ms de silêncio cortados antes do STT", removed as u64 * 1_000 / u64::from(DEFAULT_PCM_SAMPLE_RATE));
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// Whisper, fed noise, writes caption annotations ("[Música]", "(risos)", "♪") that nobody said;
/// an annotation-only transcript is no speech, not a message for the Mika to answer.
fn without_annotations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut closing: Option<char> = None;
    for ch in text.chars() {
        match (closing, ch) {
            (None, '[') => closing = Some(']'),
            (None, '(') => closing = Some(')'),
            (Some(end), c) if c == end => closing = None,
            (Some(_), _) | (None, '♪' | '♫') => {}
            (None, c) => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unsupported_provider_response(provider: &str) -> Value {
    json!({
        "success": false,
        "error": format!(
            "Provedor de STT '{provider}' não é suportado. mika:voice-transcribe transcreve com \
             Whisper local (ort) ou nuvem (Groq/OpenAI); troque para um desses em Configurações > \
             Voz."
        ),
    })
}

/// Mesma checagem de `isPcm16Mime` (`stt-provider-runner.ts`): só o prefixo `audio/pcm` indica
/// PCM16 cru. `pub(super)`: `voice::stt_cloud::build_upload` reusa esta MESMA checagem em vez de
/// duplicá-la — o gate que decide "isto é PCM16?" tem de ser um só, local e nuvem.
pub(super) fn is_pcm16_mime(mime_type: &str) -> bool {
    mime_type.starts_with("audio/pcm")
}

fn non_pcm_response(mime_type: &str) -> Value {
    let shown = if mime_type.is_empty() { "sem mimeType" } else { mime_type };
    json!({
        "success": false,
        "error": format!(
            "Whisper local espera PCM16 a {DEFAULT_PCM_SAMPLE_RATE} Hz e recebeu \"{shown}\". \
             O renderer decodifica antes de enviar."
        ),
    })
}

/// Extrai `rate=N` de `audio/pcm;rate=16000`; sem o parâmetro (ou malformado), assume o único rate
/// que o renderer já usa hoje. Não valida — `inference::whisper::transcribe` é quem recusa, alto e
/// explícito, qualquer valor que não seja 16 kHz (contrato PCM16 mono 16 kHz, AGENTS.md §2).
fn pcm_sample_rate(mime_type: &str) -> u32 {
    mime_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("rate="))
        .and_then(|rate| rate.parse::<u32>().ok())
        .unwrap_or(DEFAULT_PCM_SAMPLE_RATE)
}

/// `voice.asrLanguage` salvo; vazio (o default) = fala no idioma da interface (`app.locale`), no
/// código de duas letras que o Whisper e o Nemotron entendem. Compartilhado com o caminho de
/// barge-in (`vad_audio.rs`) para as duas transcrições nunca divergirem.
pub(crate) fn configured_language(store: &ConfigStore) -> Option<String> {
    let saved = store.get(CFG_ASR_LANGUAGE).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());
    saved.or_else(|| Some(crate::config::locale::language_code(&crate::config::locale::app_locale(store)).to_string()))
}

fn resolve_language(store: &ConfigStore, requested: Option<String>) -> Option<String> {
    requested.or_else(|| configured_language(store))
}

/// Corpo de `handle`, extraído para ser reusado por `voice::voice_tool` (ação `transcribe`) —
/// nada aqui propaga `Err`: cada falha vira `{success:false, error}`, mesma garantia do chamador.
///
/// Roteamento por provedor, MESMO ponto de dispatch para os três destinos (Whisper local, nuvem,
/// recusa nomeada) — nenhum caminho paralelo:
/// - `groq-whisper`/`openai-whisper` -> [`super::stt_cloud::transcribe`] (porte de
///   `runSttProvider`, casos `case 'groq-whisper'`/`case 'openai-whisper'`).
/// - `electron`/`local-whisper` -> Whisper local via `inference::whisper` (inalterado).
/// - qualquer outro valor (`browser`, desconhecido) -> [`unsupported_provider_response`].
/// On-device engine for `provider` (Nemotron, else Whisper), with caption annotations stripped.
/// The single local entry point: the normal STT turn and the barge-in transcription both use it.
pub(crate) async fn transcribe_local(
    provider: &str,
    whisper_model: &str,
    data_dir: PathBuf,
    pcm: Vec<u8>,
    sample_rate: u32,
    language: Option<String>,
) -> Result<Option<String>, String> {
    let pcm = cleaned_pcm(pcm, sample_rate);
    let text = if provider == "nemotron" {
        nemotron::transcribe(data_dir, pcm, sample_rate, language).await?.map(|r| r.text)
    } else {
        whisper::transcribe(data_dir, pcm, sample_rate, language, whisper_model.to_string()).await?.map(|r| r.text)
    };
    Ok(text.map(|t| without_annotations(&t)))
}

pub(crate) async fn transcribe_pcm(store: &ConfigStore, data_dir: PathBuf, req: TranscribeRequest) -> Value {
    let provider = resolve_provider(store);

    if let Some(cloud) = super::stt_cloud::CloudSttProvider::from_provider_id(&provider) {
        let mime_type = req.mime_type.as_deref().unwrap_or("").to_string();
        let sample_rate = pcm_sample_rate(&mime_type);
        let language = resolve_language(store, req.language);
        let audio = match base64::engine::general_purpose::STANDARD.decode(req.audio_base64.as_bytes()) {
            Ok(bytes) => bytes,
            Err(e) => return json!({ "success": false, "error": format!("audioBase64 inválido: {e}") }),
        };
        return super::stt_cloud::transcribe(store, cloud, audio, &mime_type, sample_rate, language).await;
    }

    if !is_local_provider(&provider) {
        return unsupported_provider_response(&provider);
    }

    let mime_type = req.mime_type.as_deref().unwrap_or("");
    if !is_pcm16_mime(mime_type) {
        return non_pcm_response(mime_type);
    }
    let sample_rate = pcm_sample_rate(mime_type);
    let language = resolve_language(store, req.language);
    let whisper_model = resolve_whisper_model(store);

    let pcm = match base64::engine::general_purpose::STANDARD.decode(req.audio_base64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(e) => return json!({ "success": false, "error": format!("audioBase64 inválido: {e}") }),
    };

    match transcribe_local(&provider, &whisper_model, data_dir, pcm, sample_rate, language).await {
        Ok(Some(text)) => json!({ "success": true, "data": { "text": without_annotations(&text) } }),
        // Áudio curto demais (< 100ms) não é erro — mesmo contrato do "no-speech" que o gate do TS
        // já devolvia como texto vazio, e não como falha.
        Ok(None) => json!({ "success": true, "data": { "text": "" } }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// Boot hook: with the mic on and a local engine selected (Nemotron or Whisper), loads the model
/// in the background so the first utterance does not wait for it.
pub fn prewarm_local_stt(app: &AppHandle) {
    let (Ok(store), Ok(data_dir)) = (crate::state::config_store(app), crate::state::app_data_dir(app)) else {
        return;
    };
    if !store.get("voice.sttEnabled").and_then(|v| v.as_bool()).unwrap_or(true) {
        return;
    }
    match resolve_provider(&store).as_str() {
        "nemotron" => drop(tauri::async_runtime::spawn_blocking(move || nemotron::prewarm(&data_dir))),
        "electron" | "local-whisper" => {
            let model_id = resolve_whisper_model(&store);
            drop(tauri::async_runtime::spawn_blocking(move || whisper::prewarm(&data_dir, &model_id)));
        }
        _ => {}
    }
}

/// Ponto de entrada chamado por `voice/mod.rs::VoiceModule::invoke` — a guarda de superfície
/// (`assertOwnerSurface`) já rodou lá antes de chegar aqui.
///
/// Fora de `parse_payload`, este handler nunca propaga `Err`: cada falha (provedor não portado,
/// mimeType errado, base64 inválido, asset do Whisper ausente, `app_data_dir` indisponível) vira
/// `{success:false, error}` — mesmo contrato de `voiceTranscribe` em `ipc.ts`, que também envolve
/// tudo num `try/catch` e nunca deixa a promessa do IPC rejeitar por uma falha de ASR.
pub async fn handle(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let req = parse_payload(payload)?;
    // "Pausar mic e tela" (bandeja): nenhum áudio vira texto, nem local nem na nuvem.
    if crate::modules::privacy::senses::paused() {
        return Ok(json!({ "success": false, "error": "mic paused" }));
    }

    let store = match crate::state::config_store(app) {
        Ok(s) => s,
        Err(e) => return Ok(json!({ "success": false, "error": e })),
    };

    let data_dir = match crate::state::app_data_dir(app) {
        Ok(d) => d,
        Err(e) => return Ok(json!({ "success": false, "error": format!("não resolveu app_data_dir: {e}") })),
    };

    let engine = store.get(CFG_VAD_ENGINE).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let ignore_typing = super::speech_gate::ignore_typing(&store);
    let (window_ms, now) = (pcm_window_ms(req.audio_base64.len()), now_ms());
    let activity = super::user_speech_activity::global();
    let verdict = activity.heard_voice_within(window_ms, now);
    if neural_vad_rejects(&engine, ignore_typing, verdict, activity.heard_keystrokes_within(window_ms, now)) {
        log::debug!("[STT] trecho descartado pelo VAD neural (Silero)");
        return Ok(json!({ "success": false, "error": NEURAL_VAD_NO_SPEECH }));
    }

    Ok(transcribe_pcm(&store, data_dir, req).await)
}

#[cfg(test)]
mod tests;
