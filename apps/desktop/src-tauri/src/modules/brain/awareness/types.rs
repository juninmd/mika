//! Tipos do motor proativo — port de `proactive-types.ts`.
//!
//! `due()` é SÍNCRONO aqui, ao contrário do `due(now): Promise<...>` do TS: os três providers
//! portados nesta rodada (ocioso, jogo, tela) só leem estado já em memória/SQLite local, sem I/O de
//! rede — `async` ficaria sem um chamador que precisasse dele de verdade.

/// Um candidato a fala proativa — já com o prompt PRONTO. Os prompts deste porte são templates
/// simples embutidos em Rust (`prompts.rs`), não o `loadPrompt` markdown do TS — ver o cabeçalho de
/// `prompts.rs` para o porquê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProactiveCandidate {
    pub key: String,
    pub priority: u8,
    pub label: String,
    pub prompt: String,
}

/// Port de `ProactiveProvider.due(now)` — um candidato por tick, ou nenhum.
pub trait ProactiveProvider: Send + Sync {
    #[allow(dead_code)] // usado por diagnóstico/teste; nenhum chamador de produção lê o id ainda.
    fn id(&self) -> &'static str;
    fn due(&self, now_ms: u64) -> Option<ProactiveCandidate>;
}
