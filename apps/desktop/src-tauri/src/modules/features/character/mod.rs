//! Feature `character` em Rust — port da tool `avatar`
//! (`apps/desktop/src/features/character/tools/avatar.ts`, ações `expression`/`inspect_model`/
//! `speak`/`affinity_delta`).
//!
//! `config/schemas/character.rs` (outra frente) já porta o namespace `character` inteiro —
//! inclusive `affinityScore` — então este módulo não abre schema nenhum.
//!
//! ## Tabela TS × Rust (`character.service.ts` lido inteiro)
//!
//! | Ação | Depende de | Rust hoje |
//! |---|---|---|
//! | `affinity_delta` | só `ConfigStore` (`CharacterService.setAffinityDelta`) | **portado** |
//! | `speak` | `EventKey::SpeechRequest` + `voice.ttsEnabled` | **portado** |
//! | `expression` | `EventKey::Expression` até o renderer (Live2D/VRM vivo) | **portado** |
//! | `inspect_model` | ler e parsear `.model3.json`/`.vrm` (Live2D/glTF) do disco | **portado** |
//!
//! `affinity_delta` é puramente config: lê/escreve `character.affinityScore` direto, sem conversão
//! de escala (diferente de `mood`/`affinity` — ver `features/mood/persistence.rs` — que reescala
//! -100..100 para 0..100; `avatar.affinity_delta` trabalha na MESMA escala -100..100 que o
//! `ConfigStore` já guarda, igual a `CharacterService.setAffinityDelta` no TS).
//!
//! `speak` publica no barramento de processo exatamente como `CharacterService.speak`.
//!
//! **`expression` fechou nesta rodada porque o bloqueio nomeado antes ("evento até o renderer")
//! caiu.** `EventKey::Expression` (`"avatar:expression"`) já existia em `crate::events::keys`, sem
//! publicador — `expression_action.rs::run_expression` é o publicador, agendando a reversão para
//! `neutral` igual
//! a `CharacterService.setExpression`/`scheduleRevert`. `expression_bridge.rs` é a ponte nova
//! (mesmo formato de `modules/chat/assistant_bridge.rs`) que encaminha `EventKey::Expression` para
//! a janela certa: `avatar:expression` está FORA de `ALL_WINDOW_EVENTS` no TS
//! (`renderer-event-bridge.ts`), então o alvo é a janela `main` quando existe, senão toda janela
//! que não seja `plugin-view` — nunca `app.emit` (alcançaria `plugin-view`, superfície não
//! confiável). O renderer já escuta: `preload/mika-runtime-chat.ts::onAvatarExpression` +
//! `renderer/src/hooks/useMikaEvents.ts`.
//!
//! **`inspect_model` fechou porque a lacuna nomeada ("crate de parser não declarada") não se
//! sustentava.** `.model3.json` é JSON puro (`serde_json` já é dependência do crate); `.vrm` é
//! glTF binário, mas só o CABEÇALHO (12 bytes fixos + 8 de comprimento/tipo do primeiro chunk)
//! importa aqui — os MESMOS bytes que `readGltfJsonChunk` (TS) já lia à mão, sem lib de glTF do
//! lado TypeScript também. `model_inventory.rs` faz o parsing puro; `inspect_model.rs` resolve o
//! arquivo em disco (`CARGO_MANIFEST_DIR` em dev/teste — ver o cabeçalho de lá para o que falta em
//! build EMPACOTADO, que é uma lacuna de EMPACOTAMENTO, não de parser).

mod affinity_action;
mod expression_action;
mod expression_bridge;
mod feature;
mod inspect_model;
pub(crate) mod model_inventory;
mod plugin_test;
pub mod slang_context;
mod slang_dictionary;
mod speak_action;
mod tool;
mod vrm_gltf;

pub(crate) use expression_bridge::spawn_expression_bridge;
pub use tool::register;
