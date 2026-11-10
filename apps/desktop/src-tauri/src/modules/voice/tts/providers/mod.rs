//! Um submódulo por provedor de TTS com gerador Rust real: `supertonic` (local), `piper` (local,
//! via subprocesso) e `remote` (ElevenLabs/OpenAI/Azure/Google, os quatro provedores de nuvem
//! HTTP). O corte nomeado que sobra (Kokoro) está documentado em `../dispatch.rs`, não aqui: um
//! `mod.rs` com stub vazio para ele seria a mesma dívida disfarçada de estrutura.

pub mod piper;
pub mod remote;
pub mod supertonic;
