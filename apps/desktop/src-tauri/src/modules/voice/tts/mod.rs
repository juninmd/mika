//! Porte do TTS local — `apps/desktop/src/main/services/voice/audio/tts-speaker.ts` e
//! `.../voice/tts/**` (orquestração, config, cache, pré-processamento) mais
//! `.../inference/supertonic.runtime.ts` (o pipeline Supertonic em si). Frente isolada da migração
//! Tauri: `Cargo.toml`/`Cargo.lock` + este diretório são o escopo inteiro, ver `AGENTS.md`.
//!
//! ## Por que isto existe: destravar `voice:vad-audio`
//!
//! `vad_pipeline.rs` (rodada anterior) já prova que `InterruptionDetector`/`BargeInRecorder` recebem
//! toda janela de VAD sem perda — o que faltava era alguém chamando
//! `InterruptionDetector::on_speech_started`/`on_speech_finished`, que só o TTS sabe quando
//! disparar. [`speaker::speak`] chama os dois (via [`speaker::SpeechObserver`]) ao redor de toda
//! síntese, sucesso ou erro — ver o cabeçalho de `speaker.rs` para os detalhes e por que é um trait
//! injetável em vez de uma chamada direta a `VadPipeline`.
//!
//! **`voice:vad-audio` foi reivindicado em 2026-09-21** (`modules/voice/vad_audio.rs`) — ver "o que
//! falta" abaixo para o que mudou desde esta rodada. [`bus_bridge`] fechou a metade que cabia neste
//! diretório: `EventBusObserver`/`EventBusSink` publicam `SpeechStarted`/`SpeechFinished`/
//! `TtsAudioReady` de verdade em qualquer `EventBus` injetado (produção passa
//! `crate::events::bus()`, o singleton do processo), e `spawn_force_stop_listener` assina
//! `ForceStopTts` nesse mesmo barramento e aciona `abort_registry::force_stop_tts()` de verdade —
//! `modules/mod.rs::all()` já chama `spawn_force_stop_listener` uma vez no boot. A ponta que faltava
//! era quem PUBLICASSE `ForceStopTts` de verdade; `vad_audio.rs` é essa ponta.
//!
//! ## Tabela de paridade TS × Rust
//!
//! | Peça TS | Rust | Status |
//! |---|---|---|
//! | `tts-speaker.ts` (`speakThroughTTS`/`speakChunks`/`processSingleChunk`) | `speaker.rs` | portado |
//! | `tts-abort-registry.ts` (`abortActiveTTS`/`registerActiveTTSController`) | `abort_registry.rs` | portado — `force_stop_tts()` é a função pública que `vad_pipeline.rs` está esperando |
//! | `preprocessor.ts` (`sanitizeText`/`normalizeLaughter`/`splitIntoSentences`) | `preprocessor.rs` | portado, exceto `translateText` (ver abaixo) |
//! | `tts-cache.ts` | `cache.rs` | portado (só a chave do Supertonic — as outras não têm gerador, ver `dispatch.rs`) |
//! | `tts-provider-dispatch.ts` (`generateWithProvider`) | `dispatch.rs` | portado — Supertonic + os 4 provedores remotos geram áudio de verdade; ElevenLabs sem o fallback automático para a voz local (ver o cabeçalho de `dispatch.rs`) |
//! | `inference/supertonic.runtime.ts` (pipeline ONNX) | `providers/supertonic/` | portado: 3 sessões `ort` (`text_encoder`→`latent_denoiser`×N→`voice_decoder`), tokenizer via crate `tokenizers`, download de modelo+voz via `reqwest` |
//! | `providers/{elevenlabs,openai,azure,google}.provider.ts` + `tts-provider-remote.ts`/`tts-utils.ts`/`registry.ts` | `providers/remote/` | portado: chokepoint de privacidade, timeout, cancelamento em voo e redação de segredo compartilhados em `remote/http.rs`; sem o cache `lastFailure`/`lastError()` de `registry.ts` (sem chamador Rust, ver o cabeçalho de `remote/mod.rs`) |
//! | `inference/kokoro.runtime.ts` + `kokoro-phonemes.ts` | — | **cortado**, nomeado — ver abaixo |
//! | `providers/piper.provider.ts` | — | **cortado**, nomeado — ver abaixo |
//! | `preprocessor.ts::translateText` (DeepLX) | — | **cortado**, nomeado — ver `preprocessor.rs` |
//! | `voice/audio/speech-mute.ts` (`isSpeechMuted`) | — | fora de `modules/voice/tts/` (vive em `audio/`, sem porte Rust ainda) — não lido aqui, ver `speaker.rs` |
//!
//! ### Cortes nomeados (por quê, não "faltou tempo")
//!
//! - **Kokoro**: fonemização depende de `espeak-ng` (pacote npm, WASM via Emscripten no lado TS).
//!   `Cargo.toml` já registra a decisão de não trazer `espeakng` (a rota nativa): ela passa por
//!   `bindgen`/`clang-sys`, e sem LLVM no ambiente de build a árvore INTEIRA para de compilar — não
//!   só esta frente, todas as oito rodando em paralelo. Nenhuma alternativa Rust para IPA fiel ao
//!   espeak-ng foi verificada nesta rodada.
//! - **Piper**: processo externo (binário `piper` + download de voz `.onnx`), não uma chamada HTTP
//!   como os 4 provedores de nuvem que já têm gerador — fora do escopo desta frente.
//! - **`translateText` (DeepLX)**: depende de um servidor HTTP de terceiros em `localhost:1188`,
//!   default desligado (`voice.ttsTranslateAudio = false`); sem esse servidor rodando, testar o
//!   caminho real seria "rede indisponível" mascarado de sucesso — mesmo problema que
//!   `project_fixture_baixado_vermelha_gate` já catalogou para downloads em teste.
//!
//! Selecionar um provedor cortado devolve `Err` NOMEADO (`dispatch.rs`) — nunca silêncio, nunca
//! áudio inventado. Isto é o que a regra "zero mock em produção" deste repositório pede.
//!
//! ## `voice:vad-audio` — reivindicado em 2026-09-21 (`modules/voice/vad_audio.rs`)
//!
//! Os três itens que esta seção listava como bloqueio, atualizados:
//!
//! 1. **`VadPipeline` não recebia `SpeechStarted`/`SpeechFinished`.** JÁ RESOLVIDO, antes desta
//!    rodada: `vad_pipeline.rs::apply_pending_speech_state` assina `crate::events::bus()` e drena as
//!    duas chaves a cada `process()` (ver o cabeçalho de lá para o mecanismo do "peek" não
//!    bloqueante). Chamada direta continuou reservada a `VadAudio`/`VadSample`, como este arquivo já
//!    esperava.
//! 2. **`speak()` sem chamador de produção.** PRONTA, TESTADA, NÃO LIGADA ainda —
//!    `modules/voice/speak_tool.rs::SpeakToolHandler` chama [`speaker::speak`] de verdade, com
//!    [`bus_bridge::EventBusObserver`]/[`bus_bridge::EventBusSink`] construídos sobre
//!    `crate::events::bus()`, e `speak_tool::register(store, app)` registra a tool `speak` no
//!    `ToolRegistry` nativo. Falta só UMA linha em `modules::register_config_bound_tools`
//!    (`modules/mod.rs`, fora do escopo deste diretório) chamando `voice::register(Arc::clone(&store),
//!    app)` — mesma classe de lacuna que `vision::register` documentou antes de ser ligada. Até essa
//!    linha entrar, `SpeechStarted`/`SpeechFinished` continuam sem publicador em produção.
//! 3. **`ForceStopTts` sem publicador de produção.** RESOLVIDO nesta rodada: `vad_audio.rs` chama
//!    `VadPipeline::on_force_stop_tts` E publica `EventKey::ForceStopTts` no barramento quando o
//!    barge-in dispara — [`bus_bridge::spawn_force_stop_listener`] (já ligado em
//!    `modules/mod.rs::all()`, chamado uma vez no boot) assina essa chave e aciona
//!    `abort_registry::force_stop_tts()` de verdade. O TTS local cala de fato quando o usuário fala
//!    por cima da Mika.
//!
//! O item 2 é o único que falta — e falta só o fio, não a peça. O catálogo da tool `speak` também é
//! menor que o TS: hoje só `text` (a ação `speak` de `tools/voice.ts`) — `listen_start`/
//! `listen_stop`/`transcribe`/`mute` continuam sem porte Rust, fora do escopo de `modules/voice/tts/`.

// O `#![allow(dead_code)]` que cobria este diretório inteiro SAIU. Ele existia porque
// `voice::register` não tinha chamador — sem essa linha em `modules::register_config_bound_tools`,
// o dead-code analysis do binário não alcançava um único item pub daqui a partir de `main`, e
// remover o allow encheria o build de warning por uma lacuna de outro arquivo.
//
// A linha entrou, e com ela o allow deixou de ser necessário: dos ~40 itens públicos deste
// diretório, exatamente DOIS continuam sem chamador, e os dois carregam `#[allow(dead_code)]`
// próprio com o motivo ao lado (`abort_registry::has_active_tts` e `SpeakResult::cancelled`).
//
// A troca importa: um allow de diretório não distingue "ainda não ligado" de "ficou órfão depois de
// uma refatoração", e este repositório já perdeu tempo com módulo portado, testado e ligado a nada.
// Um allow por item mantém o resto do diretório sob vigilância.

pub mod abort_registry;
pub mod bus_bridge;
pub mod cache;
pub mod dispatch;
pub mod preprocessor;
pub mod providers;
pub mod speaker;
pub mod speech_mute;
pub mod state;
pub(crate) mod tool_call_text;
