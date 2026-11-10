//! Feature `desktop` em Rust — port de `apps/desktop/src/features/desktop/tools/index.ts`, as 3
//! ações `launch`/`window`/`process`, registrada de verdade no `ToolRegistry` nativo
//! (`crate::modules::tools`).
//!
//! **Não confundir com `modules/desktop.rs`.** Aquele arquivo porta outra coisa que só tem o
//! MESMO nome por coincidência — auto-launch do SO (`login-items.ts`) e download de assets do
//! wizard (`wizard-assets.ts`). Nenhum dos dois cobre o outro; este diretório é a feature tardia
//! `apps/desktop/src/features/desktop/` (a tool que lê janelas/processos e abre app/site).
//!
//! Diretório, não arquivo único, só pela regra de 300 linhas do AGENTS.md: [`launch_target`] guarda
//! a resolução de alvo, [`launch`] a ação `launch` (as 3 barreiras — formato, denylist, modo
//! offline), [`process`] a listagem via `sysinfo`, [`window`] a listagem via `xcap`, e [`tool`] a
//! tool `desktop` que o LLM chama, costurando as quatro.
//!
//! ## As DUAS premissas do cabeçalho anterior caíram — medidas nesta rodada, não presumidas
//!
//! 1. **`action=window` "precisa de shell-out".** Nunca precisou: o TS só usa
//!    `powershell -EncodedCommand` porque o Node não tem API nativa de enumeração de janela — o
//!    Rust já tinha uma via `xcap` (dependência do crate desde a captura de tela, `capture.rs`).
//!    E a redação de PII que o cabeçalho antigo dizia esbarrar em "look-behind sem suporte" TAMBÉM
//!    já tinha porte: `features::person_catalog::pii::redact_pii` (existente, testado contra o TS
//!    linha a linha) resolve os dois padrões de look-behind com um filtro pós-match, e é a mesma
//!    função reaproveitada aqui — ver [`window`]. **Melhor que o original**: funciona nos três SOs
//!    (Windows/macOS/Linux), não só Windows — o script PowerShell era a limitação, não o produto,
//!    mesma nota que [`process`] já fazia para `action=process`.
//! 2. **`action=launch` "a tool não tem `AppHandle`".** Era verdade quando escrito; deixou de ser
//!    em 2026-09-21 (`ebb5d7eb`): `modules::register_config_bound_tools(app)` (`modules/mod.rs`) é
//!    exatamente a SEGUNDA passada de registro, com `AppHandle`/`ConfigStore`, que `vision`,
//!    `user`, `mood`, `character`, `scheduler`, `person_catalog`, `privacy` e `voice` já usam. Este
//!    módulo estava fora dela por uma decisão de rodada anterior, não por impossibilidade técnica —
//!    ver [`register`] (`tool.rs`), a função nova que fecha esse buraco.
//!
//! ## Duas passadas de registro, mesmo padrão de `person_catalog`
//! [`register_tools`] (primeira passada, sem `AppHandle`, efeito colateral de `features::modules()`
//! — já chamada em produção, sem mudança necessária ali) registra `process`/`window` já
//! despachando de verdade e `launch` degradando nomeado (`errorCode: "unavailable"`).
//! [`register`] (segunda passada, com `Arc<ConfigStore>`) SUBSTITUI esse registro pelo mesmo nome
//! de tool — `ToolRegistry::register` sobrescreve por nome — desta vez com `launch` despachando de
//! verdade, gated pelo chokepoint de modo offline (`crate::modules::privacy`).
//!
//! **Duas mudanças fora deste diretório, fora do meu escopo editar, nomeadas para quem for aplicar
//! (ver o relatório desta rodada):**
//! 1. `apps/desktop/src-tauri/src/modules/features/mod.rs`: `mod desktop;` → `pub mod desktop;` —
//!    sem isso, `modules::register_config_bound_tools` (outro módulo, fora de `features`) não
//!    alcança `features::desktop::register`.
//! 2. `apps/desktop/src-tauri/src/modules/mod.rs::register_config_bound_tools`: uma linha
//!    `features::desktop::register(Arc::clone(&store));`, no mesmo bloco que já chama
//!    `features::user::register`/`features::mood::register`.
//!
//! ## Hábitos de uso
//! [`usage`] (livro puro de segundos por programa/dia) e [`habits`] (contador + tool `habits`, top
//! de programas e jogos) — ver o cabeçalho de `habits.rs` para as regras de privacidade.
//!
//! ## Política de segurança de `launch`, em uma linha
//! Allowlist de formato (URL http(s) ou nome de comando seguro) **e** denylist de interpretador
//! (`cmd`/`powershell`/`wsl`/`mshta`/...) **e** chokepoint de modo offline — as três, sempre, antes
//! de `std::process::Command`/abrir navegador; nenhum comando passa por shell, nenhuma string é
//! concatenada. Ver o cabeçalho de [`launch`] para o detalhe barreira a barreira.

pub(crate) mod habits;
mod launch;
mod launch_target;
mod process;
mod tool;
pub(crate) mod usage;
mod window;

pub(crate) use tool::{register, register_tools};
