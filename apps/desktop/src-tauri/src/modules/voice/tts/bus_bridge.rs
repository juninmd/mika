//! Porte de produção que faltava para `speak()` deixar de ser só um trait injetável: implementações
//! REAIS de [`SpeechObserver`]/[`SpeechSink`] sobre `crate::events::EventBus` (o barramento
//! cross-módulo que `events/keys.rs` ampliou nesta rodada com
//! `SpeechStarted`/`SpeechFinished`/`TtsAudioReady`/`ForceStopTts`), e o lado de CONSUMO de
//! `Events.FORCE_STOP_TTS`: [`spawn_force_stop_listener`] assina o barramento e aciona
//! [`super::abort_registry::force_stop_tts`] — a mesma função que `dispatch.rs`/`speaker/chunk.rs`
//! checam via `CancelHandle` a cada chunk.
//!
//! **Barramento injetado, não `crate::events::bus()` direto.** Todo tipo aqui recebe `&'static
//! EventBus` no construtor em vez de alcançar o singleton por dentro — mesmo padrão de
//! `vad_bridge.rs::VadStream::process` (ver o comentário de lá: "para o chamador real escolher a
//! instância; produção usa `events::bus()`, teste uma isolada"). Quem ligar isto em produção passa
//! `crate::events::bus()`; os testes deste arquivo passam uma instância isolada
//! (`EventBus::new()`), o que os tira da disputa por singleton de processo que o cabeçalho de
//! `events/bus.rs` documenta (`cargo test` roda o binário inteiro em paralelo — um teste que assina
//! `bus()` recebe eventos de QUALQUER outro teste rodando ao mesmo tempo).
//!
//! **O que isto NÃO fecha sozinho.** `voice:vad-audio` continua fora do `CHANNELS` de
//! `modules/voice/mod.rs`: nada ainda instancia uma `VadPipeline` por sessão, nada chama `speak()`
//! com [`EventBusObserver`]/[`EventBusSink`] em produção (isso é `modules/tools/`, a tool `speak`,
//! fora deste diretório), e o outro lado do `ForceStopTts` — QUEM publica quando o barge-in dispara
//! — ainda não existe (`vad_pipeline.rs::on_force_stop_tts` é chamado só em teste). Este arquivo
//! entrega as DUAS pontas que cabem no escopo de `modules/voice/tts/`: publicar
//! `SpeechStarted`/`SpeechFinished`/`TtsAudioReady` de verdade, e consumir `ForceStopTts` de verdade.
//!
//! **Payload reduzido em `SpeechStarted`/`SpeechFinished` — decisão deliberada, não corte
//! escondido.** `tts-speaker.ts` emite `{ textLength, provider, at }`, mas o único consumidor Rust
//! real (`InterruptionDetector::on_speech_started`/`on_speech_finished`, via
//! `VadPipeline::on_speech_started`/`on_speech_finished`) só aceita `now_ms` — `textLength`/
//! `provider` nunca chegam a ser lidos do lado que decide o barge-in, são puramente informativos
//! para HUD/telemetria do lado TS. Herdar esses dois campos exigiria alargar `SpeechObserver` só
//! para um consumidor que não existe em Rust ainda; quem precisar deles de volta adiciona quando
//! tiver um assinante real.

use base64::Engine;
use serde_json::json;

use crate::events::{EventBus, EventKey};

use super::abort_registry;
use super::speaker::{SpeechObserver, SpeechSink, TtsChunkAudio};

/// Publica `EventKey::SpeechStarted`/`SpeechFinished` no barramento injetado — a ponte real que
/// falta para `InterruptionDetector` saber que a Mika está falando, uma vez que algo (a tool
/// `speak`, fora deste diretório) chame `speak()` com esta instância construída sobre
/// `crate::events::bus()`.
pub struct EventBusObserver {
    bus: &'static EventBus,
}

impl EventBusObserver {
    pub fn new(bus: &'static EventBus) -> Self {
        Self { bus }
    }
}

impl SpeechObserver for EventBusObserver {
    fn on_speech_started(&self, now_ms: u64) {
        self.bus.publish(EventKey::SpeechStarted, json!({ "atMs": now_ms }));
    }

    fn on_speech_finished(&self) {
        self.bus.publish(EventKey::SpeechFinished, json!({}));
    }
}

/// Publica `EventKey::TtsAudioReady` por chunk — equivalente a `Events.TTS_AUDIO_READY` de
/// `tts-speaker.ts`. `words` (alinhamento por palavra) fica de fora: o pipeline Supertonic portado
/// (`providers/supertonic/model.rs`) não produz timestamps por palavra, só a forma de onda — cortar
/// esse campo aqui é reportar a mesma ausência, não inventar um valor.
pub struct EventBusSink {
    bus: &'static EventBus,
}

impl EventBusSink {
    pub fn new(bus: &'static EventBus) -> Self {
        Self { bus }
    }
}

impl SpeechSink for EventBusSink {
    fn on_chunk_ready(&self, chunk: TtsChunkAudio) {
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&chunk.audio);
        self.bus.publish(
            EventKey::TtsAudioReady,
            json!({
                "audio": audio_base64,
                "mimeType": chunk.mime_type,
                "provider": chunk.provider,
                "chunkIndex": chunk.chunk_index,
                "totalChunks": chunk.total_chunks,
            }),
        );
    }
}

/// Assina `EventKey::ForceStopTts` no barramento passado e aciona `abort_registry::force_stop_tts`
/// — o lado de CONSUMO da atuação de barge-in. Quem PUBLICA esta chave (o barge-in real, disparado
/// por `VadPipeline`/`InterruptionDetector`) ainda não existe fora de teste — ver o cabeçalho deste
/// arquivo. `tokio::spawn` porque `EventSubscription::recv` é um laço bloqueante em `.await`: não
/// pode rodar inline no boot, precisa da própria task (mesmo padrão de `mcp/client.rs`).
pub fn spawn_force_stop_listener(bus: &'static EventBus) {
    // Assina ANTES de `tokio::spawn`, não dentro do bloco async: `broadcast::Sender::subscribe`
    // só enxerga mensagens publicadas DEPOIS da assinatura, e `tokio::spawn` só agenda a task —
    // não garante que ela já rodou até o chamador ceder o turno (`.await`). Assinar dentro do
    // bloco async deixaria uma janela onde um `publish(ForceStopTts, ..)` chamado logo depois
    // desta função nunca chegaria: a task ainda nem tinha se inscrito quando a mensagem passou.
    let mut sub = bus.subscribe();
    // `tauri::async_runtime::spawn`, NUNCA `tokio::spawn`: esta função roda em `lib.rs::setup`, que
    // o Tauri chama FORA do contexto do reator. `tokio::spawn` ali panica na hora com "there is no
    // reactor running, must be called from the context of a Tokio 1.x runtime", e o processo morre
    // com exit 101 antes de abrir janela. Ver o gate em `modules/mod.rs`.
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            match envelope.key {
                EventKey::ForceStopTts => {
                    // Devolve se havia síntese em voo; aqui não muda decisão nenhuma.
                    let _ = abort_registry::force_stop_tts();
                }
                // O outro lado da ação `mute`. A tabela de paridade de `voice_tool.rs` dizia
                // "portado" porque o evento era publicado com o payload certo — e ninguém o lia:
                // mandar a Mika calar a boca devolvia sucesso e ela continuava falando.
                EventKey::MuteSpeech => super::speech_mute::apply(&envelope.payload),
                _ => {}
            }
        }
    });
}

// `spawn_speech_request_listener` MUDOU DE DONO nesta rodada: `EventKey::SpeechRequest` agora é
// consumido por `voice::auto_speak::spawn_auto_speak` (ver o cabeçalho daquele módulo), no MESMO
// laço serial que fala o turno de chat — dois assinantes independentes chamando
// `speak_tool::run_speak` ao mesmo tempo era exatamente o risco de sobreposição de áudio que a
// versão anterior deste arquivo evitava "serializando o run_speak dentro do laço", só que
// serializado DENTRO de cada assinante, não ENTRE os dois. `speech_request_text` foi portado junto
// (mesma lógica pura, mesmo teste) — não sobra função nem teste aqui.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn observer_publishes_speech_started_and_finished_with_now_ms() {
        let bus = Box::leak(Box::new(EventBus::new()));
        let observer = EventBusObserver::new(bus);
        let mut sub = bus.subscribe();

        observer.on_speech_started(42);
        let envelope = sub.recv().await.expect("devia publicar");
        assert_eq!(envelope.key, EventKey::SpeechStarted);
        assert_eq!(envelope.payload["atMs"], json!(42));

        observer.on_speech_finished();
        let envelope = sub.recv().await.expect("devia publicar");
        assert_eq!(envelope.key, EventKey::SpeechFinished);
    }

    #[tokio::test]
    async fn sink_encodes_audio_as_base64_with_mime_and_chunk_metadata() {
        let bus = Box::leak(Box::new(EventBus::new()));
        let sink = EventBusSink::new(bus);
        let mut sub = bus.subscribe();

        sink.on_chunk_ready(TtsChunkAudio {
            audio: vec![1, 2, 3, 4],
            mime_type: "audio/wav",
            chunk_index: 0,
            total_chunks: 2,
            provider: "supertonic".into(),
        });

        let received = sub.recv().await.expect("devia publicar");
        assert_eq!(received.key, EventKey::TtsAudioReady);
        assert_eq!(received.payload["mimeType"], "audio/wav");
        assert_eq!(received.payload["chunkIndex"], 0);
        assert_eq!(received.payload["totalChunks"], 2);
        assert_eq!(
            received.payload["audio"],
            base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4])
        );
    }

    /// Prova de ponta a ponta da metade que ESTE arquivo fecha: publicar `ForceStopTts` num
    /// barramento ISOLADO aciona `abort_registry::force_stop_tts()` de verdade, sem passar por
    /// `VadPipeline` (que ainda não publica essa chave em produção — ver o cabeçalho do arquivo).
    /// Isolado porque `abort_registry::global()` também é singleton de processo (mesmo registro
    /// que `speaker::speak()` usa em produção); um barramento isolado ainda assim prova a fiação
    /// REAL do listener (assinatura -> filtro de chave -> chamada), só não a instância `bus()`.
    #[tokio::test]
    async fn force_stop_listener_reacts_to_a_bus_publish() {
        let bus = Box::leak(Box::new(EventBus::new()));
        spawn_force_stop_listener(bus);

        let registry = abort_registry::global();
        let guard = registry.register_active();
        let handle = guard.handle();

        bus.publish(EventKey::ForceStopTts, json!({}));

        // O assinante roda em outra task; espera cooperativamente em vez de assumir ordem
        // síncrona — `tokio::spawn` só agenda, `.await`/`sleep` é quem cede o turno para ela rodar.
        for _ in 0..200 {
            if handle.is_cancelled() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(handle.is_cancelled(), "force_stop_tts não chegou a cancelar o handle ativo");
    }
}
