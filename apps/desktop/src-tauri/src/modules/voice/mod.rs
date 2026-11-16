//! Módulo `voice` em Rust — port PARCIAL do backend TypeScript correspondente.
//!
//! **Tool `voice` fechada nesta rodada (`voice_tool.rs`).** Até aqui só a tool `speak` (atalho de
//! produção para `action=speak`) estava registrada; `mika:tools-list` não tinha a tool `voice`
//! inteira que o catálogo TS declara (`voice.feature.ts`). `voice_tool.rs` registra as 5 ações —
//! `speak`/`mute`/`transcribe` reusam os mesmos caminhos já testados (`speak_tool::run_speak`,
//! `transcribe::transcribe_pcm`, `EventKey::MuteSpeech`); `listen_start`/`listen_stop` agora
//! despacham de verdade sobre [`listening_state::ListeningState`] — o flag `STTService.listening`
//! do TS, que nunca iniciava captura de áudio no Node também (ver o cabeçalho de `voice_tool.rs`
//! e de `listening_state.rs` para a leitura completa do porquê isto fecha a ação sem inventar um
//! motor de STT contínuo em Rust).
//!
//! 4 dos 6 canais candidatos: `voice:mic-state`, `voice:mic-state-request` (aqui),
//! `mika:voice-transcribe` (`transcribe.rs`) e `voice:vad-audio` (`vad_audio.rs`, ligado nesta
//! rodada — o último canal de voz que faltava). Ver
//! `apps/desktop/src/main/services/voice/ipc.ts::registerVoiceIPC` para o comportamento
//! original, e `docs/tauri-rust-migration.md` (fase 5) para a ordem da migração.
//!
//! **`mika:voice-transcribe` — destravado numa rodada anterior.** `inference/whisper.rs` já roda o
//! pipeline inteiro (espectrograma, encoder, decoder autoregressivo, tokenizer) sobre `ort`,
//! verificado contra áudio real. O bloqueio que restava era `modules/desktop.rs::WHISPER_FILES`
//! nunca baixar `tokenizer.json` — `whisper::require_asset` falhava sempre por causa disso.
//! `desktop.rs::prepare_whisper` agora baixa os três arquivos (dois pesos ONNX + tokenizer), então
//! `transcribe.rs` liga o canal ao Whisper local de verdade. Escopo do que foi portado, e o que
//! ficou de fora, no cabeçalho de `transcribe.rs`.
//!
//! **`voice:vad-audio` — reivindicado nesta rodada.** As duas lacunas que a rodada anterior
//! apontava como bloqueio já caducaram antes desta: `voice::tts::bus_bridge::EventBusObserver`
//! publica `SpeechStarted`/`SpeechFinished` de verdade (falta só um chamador de `speak()` em
//! produção, fora do escopo de voz — ver `tts/mod.rs`), e `chat::turns::abort_turn` já existia,
//! público, sem chamador. `vad_audio.rs` é esse chamador: um `VadPipeline` de processo inteiro que
//! janela o payload cru em blocos de `FRAME_SAMPLES`, publica `VadAudio` sempre (nunca descartado),
//! `VadSample` a no máximo 100 Hz, e — no barge-in — publica `ForceStopTts` E aborta o turno em
//! curso da janela chamadora. Detalhe completo no cabeçalho de `vad_audio.rs`.
//!
//! **NÃO reivindicados:**
//!
//! - `shortcut:toggle-mic` / `shortcut:toggle-input`: no Electron (`main/shell/app-shortcuts.ts`)
//!   estes NÃO são canais que o renderer invoca — são eventos que o MAIN emite quando F9 /
//!   `CommandOrControl+Space` disparam (`win.webContents.send(channel)`), a partir de um
//!   `globalShortcut.register` feito uma vez no boot (`registerMainWindowShortcuts`,
//!   `main/index.ts`). O mecanismo de porte deste módulo é `NativeModule::invoke`, que só responde
//!   a chamada VINDA do renderer (`mika_invoke`/`mika_send`) — não há gancho aqui para "registrar
//!   isto uma vez quando o app sobe": isso é `lib.rs`/`.setup()`, fora do único arquivo desta
//!   tarefa. `shortcuts.rs` já cobre o registro DINÂMICO (usado por atalhos configuráveis, como
//!   mute); F9/Space são HARDCODED e authorize permanente do app, categoria diferente. Registrar
//!   os dois aqui sem o AppHandle do boot seria inventar um segundo ponto de entrada não pedido.
//!   Relatado para quem tiver `lib.rs` no escopo.
//!
//! **Isolamento da inferência: já implementado, não mais uma recomendação.** `inference/vad.rs` e
//! `inference/whisper.rs` já rodam toda chamada síncrona de `ort` (`Session::run`) dentro de
//! `tokio::task::spawn_blocking` — mesma garantia que o `utilityProcess` do Node dava (o Node
//! isola porque `onnxruntime-node` é síncrono e bloquearia o único thread de JS), sem o custo de
//! IPC de um processo separado. O risco real continua sendo travar o worker que também resolve
//! `mika_invoke`, não o processo inteiro.

mod auto_speak;
pub(crate) use auto_speak::speech_text::strip_emotion_tags;
mod barge_in_recorder;
mod expression_sync;
pub mod feature;
mod interruption_detector;
mod interruption_transcript;
mod listen_actions;
mod listening_state;
mod pcm_wav;
mod silence_command;
mod speak_guard;
mod speak_tool;
mod speech_gate;
mod stt_cleanup;
#[cfg(test)]
mod stt_smoke;
// `pub(crate)` só por causa de `stt_cloud::validate_credentials`, que a tela de Configurações
// chama para o botão "testar conexão" — o resto do módulo continua interno a `voice`.
pub(crate) mod stt_cloud;
mod transcribe;
pub mod tts;
// `pub` só porque `brain::awareness::engine` lê o tracker de atividade de voz para gatear a fala
// proativa (defects.md "barge-in", item 3) — o resto do módulo segue interno a `voice`.
pub mod user_speech_activity;
pub mod vad_audio;
mod vad_bridge;
mod vad_pipeline;
mod voice_tool;

/// A fala automática de chat — quem liga `voice.autoSpeak` ao runtime real (ver o cabeçalho de
/// `auto_speak/mod.rs`). Chamada por `modules::register_config_bound_tools`, mesmo ponto de
/// `speak_tool::register`/`tts::bus_bridge::spawn_speech_request_listener` (precisa de
/// `ConfigStore` e de `cache_dir`, resolvido a partir do `AppHandle`).
pub use auto_speak::spawn_auto_speak;
/// Port de `ExpressionSyncService` — publica `EventKey::ActivityChanged` a cada transição de voz
/// (ver o cabeçalho de `expression_sync.rs`). Sem chamador de config/`AppHandle`, então
/// `modules::register_config_bound_tools` chama isto direto, ao lado de `spawn_auto_speak`.
pub use expression_sync::spawn_expression_sync;
pub use speak_tool::{prewarm_local_tts, register};
pub use transcribe::prewarm_local_stt;
/// A tool `voice` inteira (TS `tools/voice.ts`, 5 ações) — distinta da tool `speak` acima (só
/// `action=speak`, registrada antes desta existir). Nome de reexport diferente para não colidir
/// com [`register`].
pub use voice_tool::register as register_voice_tool;
/// Reexport estreito para `features::privacy::clear_history` limpar o MESMO cache de TTS que as
/// tools `speak`/`voice` usam — dois `OnceLock` independentes deixariam a fala já cacheada viva
/// mesmo depois do "apagar tudo".
pub(crate) use speak_tool::cache as tts_cache;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

const CHANNELS: [&str; 5] = [
    "voice:mic-state",
    "voice:mic-state-request",
    "mika:voice-transcribe",
    "voice:vad-audio",
    "mika:voice-silence-command",
];

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.HudMicState` — o HUD do microfone em toda janela viva.
const HUD_MIC_STATE_EVENT: &str = "hud:mic-state";

/// Mesmos números de `ipc.ts` (`MIC_STATE_WINDOW_MS`, `MIC_STATE_MAX_CHANGES`): mais de 10
/// mudanças REAIS de estado por segundo é tempestade no renderer, não intenção do usuário.
const MIC_STATE_WINDOW: Duration = Duration::from_millis(1000);
const MIC_STATE_MAX_CHANGES: u32 = 10;

/// Dedup + circuit-breaker do mic-state, espelhando `ipc.ts::isMicStateSuppressed`.
///
/// A versão Node também reconciliava com o estado REAL do STT (`currentListeningState()`), porque
/// ligar/desligar o microfone ali também liga/desliga a escuta de verdade
/// (`runStartListening`/`runStopListening`). Aqui não: STT continua no sidecar (ver o cabeçalho
/// deste arquivo), então não existe "estado real de escuta" em Rust para comparar — só o dedup por
/// valor e o limite de rajada sobrevivem. Isto sincroniza o HUD com o que o usuário apertou; NÃO
/// liga nem desliga a escuta. Degradação relatada aqui, não escondida atrás de um retorno "ok".
struct MicStateBreaker {
    last_enabled: bool,
    window_started_at: Instant,
    window_count: u32,
    tripped: bool,
}

impl MicStateBreaker {
    fn new() -> Self {
        Self {
            last_enabled: false,
            window_started_at: Instant::now(),
            window_count: 0,
            tripped: false,
        }
    }
}

/// `true` = ignora (valor repetido, ou tempestade de alternâncias dentro da janela).
fn mic_state_suppressed(breaker: &mut MicStateBreaker, enabled: bool, now: Instant) -> bool {
    if enabled == breaker.last_enabled {
        return true;
    }

    if now.duration_since(breaker.window_started_at) >= MIC_STATE_WINDOW {
        breaker.window_started_at = now;
        breaker.window_count = 0;
        breaker.tripped = false;
    }

    breaker.window_count += 1;
    if breaker.window_count <= MIC_STATE_MAX_CHANGES {
        return false;
    }

    if !breaker.tripped {
        breaker.tripped = true;
        log::warn!(
            "voiceMicState suprimido: alternâncias demais na janela ({} mudanças)",
            breaker.window_count
        );
    }
    true
}

fn extract_enabled(payload: &Value) -> Result<bool, String> {
    payload["enabled"]
        .as_bool()
        .ok_or_else(|| "payload deve ter 'enabled' booleano".to_string())
}

/// `voice:mic-state`: o renderer avisa que o usuário ligou/desligou o microfone. Atualiza o
/// dedup/breaker e, só quando o valor realmente mudou (e não foi suprimido), transmite o novo
/// estado para o HUD de TODA janela viva — mesmo contrato de `broadcastMicState` em `ipc.ts`.
fn handle_mic_state(app: &AppHandle, breaker: &Mutex<MicStateBreaker>, payload: &Value) -> Result<Value, String> {
    let enabled = extract_enabled(payload)?;

    let should_broadcast = {
        let mut guard = breaker
            .lock()
            .map_err(|_| "lock do estado do microfone envenenado".to_string())?;
        if mic_state_suppressed(&mut guard, enabled, Instant::now()) {
            false
        } else {
            guard.last_enabled = enabled;
            true
        }
    };

    if should_broadcast {
        let _ = app.emit(HUD_MIC_STATE_EVENT, json!({ "enabled": enabled }));
    }
    Ok(json!({ "ok": true }))
}

/// `voice:mic-state-request`: o renderer que acabou de montar pede o estado atual. Responde só à
/// janela que pediu (`emit_to`), não a todas — mesmo contrato de `voiceMicStateRequest` em
/// `ipc.ts`, que manda pelo `webContents` da janela chamadora.
fn handle_mic_state_request(app: &AppHandle, ctx: &InvokeContext, breaker: &Mutex<MicStateBreaker>) -> Result<Value, String> {
    let enabled = breaker
        .lock()
        .map_err(|_| "lock do estado do microfone envenenado".to_string())?
        .last_enabled;

    let _ = app.emit_to(ctx.window_label.as_str(), HUD_MIC_STATE_EVENT, json!({ "enabled": enabled }));
    Ok(json!({ "ok": true }))
}

struct VoiceModule {
    mic: Mutex<MicStateBreaker>,
}

#[async_trait]
impl NativeModule for VoiceModule {
    fn name(&self) -> &'static str {
        "voice"
    }

    fn channels(&self) -> &'static [&'static str] {
        &CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        // Mesma guarda de `assertOwnerSurface(context, 'voiceMicState' | 'voiceMicStateRequest' |
        // 'voiceTranscribe' | 'voiceVadAudio')` em `ipc.ts`: só `main`/`settings`/`wizard` mexem no
        // microfone — sem isso qualquer superfície (inclusive uma view de plugin) empurraria áudio
        // para dentro do VAD.
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }

        match channel {
            "voice:mic-state" => handle_mic_state(app, &self.mic, &payload),
            "voice:mic-state-request" => handle_mic_state_request(app, ctx, &self.mic),
            "mika:voice-transcribe" => transcribe::handle(app, &payload).await,
            "voice:vad-audio" => vad_audio::handle(app, ctx, &payload).await,
            "mika:voice-silence-command" => silence_command::handle(&payload),
            other => Err(format!("canal '{other}' declarado por 'voice' mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(VoiceModule {
        mic: Mutex::new(MicStateBreaker::new()),
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".to_string(),
            plugin_id: None,
        }
    }

    /// Também a prova estrutural de "Superfície não-dona é recusada" para `voice:vad-audio`: a
    /// guarda de `is_owner_surface` roda ANTES do `match` de canal em `invoke`, uma vez só para os
    /// quatro — `voice:vad-audio` nunca escapa dela. `owner_surface_matches_ts_policy` (abaixo)
    /// prova a política em si (quem passa, quem é recusado).
    #[test]
    fn claims_exactly_the_four_declared_channels() {
        let module = VoiceModule {
            mic: Mutex::new(MicStateBreaker::new()),
        };
        assert_eq!(
            module.channels(),
            &[
                "voice:mic-state",
                "voice:mic-state-request",
                "mika:voice-transcribe",
                "voice:vad-audio",
                "mika:voice-silence-command",
            ]
        );
    }

    /// Mesmo padrão de `system.rs::channels_match_typescript_contract`: um literal errado aqui
    /// não quebra a compilação, só faz o canal nunca ser reivindicado e cair de volta pro sidecar
    /// em silêncio.
    #[test]
    fn channels_match_typescript_contract() {
        let channels_contract =
            include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");

        for channel in CHANNELS {
            assert!(
                channels_contract.contains(&format!("'{channel}'")),
                "canal {channel} não existe em desktop-ipc-channels.ts"
            );
        }
        assert!(
            channels_contract.contains(&format!("'{HUD_MIC_STATE_EVENT}'")),
            "'{HUD_MIC_STATE_EVENT}' não existe em desktop-ipc-channels.ts — o HUD nunca receberia o evento"
        );
    }

    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(is_owner_surface(ctx(WindowType::Main).window_type));
        assert!(is_owner_surface(ctx(WindowType::Settings).window_type));
        assert!(is_owner_surface(ctx(WindowType::Wizard).window_type));
        assert!(!is_owner_surface(ctx(WindowType::PluginView).window_type));
        assert!(!is_owner_surface(ctx(WindowType::Unknown).window_type));
    }

    #[test]
    fn extract_enabled_rejects_missing_field() {
        assert!(extract_enabled(&json!({})).is_err());
    }

    #[test]
    fn extract_enabled_rejects_non_bool() {
        assert!(extract_enabled(&json!({ "enabled": "yes" })).is_err());
    }

    #[test]
    fn extract_enabled_accepts_bool() {
        assert_eq!(extract_enabled(&json!({ "enabled": true })).unwrap(), true);
        assert_eq!(extract_enabled(&json!({ "enabled": false })).unwrap(), false);
    }

    #[test]
    fn mic_state_suppressed_rejects_identical_repeat() {
        let mut breaker = MicStateBreaker::new();
        let now = Instant::now();

        // Primeira mudança real (false -> true) não é suprimida.
        assert!(!mic_state_suppressed(&mut breaker, true, now));
        breaker.last_enabled = true;

        // Repetir o MESMO valor é o "voiceMicState repetido ignorado" do lado Node.
        assert!(mic_state_suppressed(&mut breaker, true, now));
    }

    #[test]
    fn mic_state_suppressed_allows_bursts_up_to_the_limit() {
        let mut breaker = MicStateBreaker::new();
        let start = Instant::now();
        let mut enabled = true;

        for _ in 0..MIC_STATE_MAX_CHANGES {
            assert!(!mic_state_suppressed(&mut breaker, enabled, start));
            breaker.last_enabled = enabled;
            enabled = !enabled;
        }
    }

    /// A alternância que passa do limite é a tempestade que o breaker existe para conter — mesmo
    /// caso que motivou `MIC_STATE_MAX_CHANGES` do lado Node.
    #[test]
    fn mic_state_suppressed_trips_after_the_limit() {
        let mut breaker = MicStateBreaker::new();
        let start = Instant::now();
        let mut enabled = true;

        for _ in 0..MIC_STATE_MAX_CHANGES {
            assert!(!mic_state_suppressed(&mut breaker, enabled, start));
            breaker.last_enabled = enabled;
            enabled = !enabled;
        }

        assert!(mic_state_suppressed(&mut breaker, enabled, start));
    }

    #[test]
    fn mic_state_suppressed_resets_after_the_window_elapses() {
        let mut breaker = MicStateBreaker::new();
        let start = Instant::now();
        let mut enabled = true;

        for _ in 0..MIC_STATE_MAX_CHANGES {
            assert!(!mic_state_suppressed(&mut breaker, enabled, start));
            breaker.last_enabled = enabled;
            enabled = !enabled;
        }
        assert!(mic_state_suppressed(&mut breaker, enabled, start));

        let later = start + MIC_STATE_WINDOW;
        assert!(!mic_state_suppressed(&mut breaker, enabled, later));
    }
}
