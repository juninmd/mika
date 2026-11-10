//! Os 4 provedores de TTS que são chamada HTTP a um serviço de nuvem — porte de
//! `providers/{elevenlabs,openai,azure,google}.provider.ts` mais o encanamento de
//! `tts-provider-remote.ts`/`providers/tts-utils.ts`/`providers/registry.ts` (fetch com timeout,
//! leitura do corpo, classificação de falha por status). `dispatch.rs` chama um `generate` por
//! provedor; `http.rs` é a infra que os 4 compartilham (chokepoint de privacidade, corrida com
//! cancelamento, redação de segredo).
//!
//! **O que NÃO foi portado, de propósito.** `registry.ts` guarda `lastFailure`/`lastError()` por
//! provedor em estado de módulo, para `TTSFactory#getLastGenerationError()` alimentar a tela depois
//! de uma síntese falhar. Nenhum chamador Rust lê isso hoje — `getLastGenerationError` não tem
//! porte, nem canal de IPC que o exponha — então guardar o estado seria o mesmo código morto que
//! `abort_registry::has_active_tts` já documenta como exceção com motivo, só que sem o motivo: aqui
//! o motivo é "ninguém pediu ainda". A mensagem de erro em si (com status e razão) continua
//! completa no `Result::Err` que `dispatch.rs` propaga — só o CACHE entre chamadas ficou de fora.
//! Mesma lógica vale para a classificação de falha PERMANENTE do ElevenLabs (`getElevenLabsPermanentFailure`,
//! `scope: 'account' | 'voice'`) e o fallback automático para a voz local: isso mora em
//! `tts-provider-dispatch.ts` do lado TS, fora da lista de arquivos-fonte desta tarefa
//! (`elevenlabs.provider.ts`, não `tts-provider-dispatch.ts`), e depende de estado observável só
//! pelo chamador de `TTSFactory` — reportado, não implementado aqui.

mod azure;
mod elevenlabs;
mod google;
mod http;
mod openai;

pub use azure::{generate as generate_azure, AzureTtsOptions};
pub use elevenlabs::{generate as generate_elevenlabs, ElevenLabsOptions};
pub use google::{generate as generate_google, GoogleTtsOptions};
pub use openai::{generate as generate_openai, OpenAiTtsOptions};
