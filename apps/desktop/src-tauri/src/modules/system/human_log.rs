//! Marca de call site para o tier humano de `specs/10-observability.md` ("Dois públicos, dois
//! tiers"): "frase completa em português, com o EFEITO explícito... não como evento cru". A
//! computação de `audience` já deriva do NÍVEL (`log_buffer::audience_for`: `info`/`warn`/`error`
//! vira `"human"`), então esta macro não muda tier nenhum — o valor dela é OUTRO: dar um nome
//! greppável ("isto aqui é um momento de decisão/sintoma, a frase precisa ter o efeito") aos
//! poucos pontos que o MUST pede, em vez de um `log::warn!` indistinguível de qualquer outro.
//!
//! **Macro, não função.** Precisa expandir NO local da chamada para `module_path!()` (usado por
//! `log::log!` internamente como `target` por padrão) continuar apontando para o módulo que decidiu
//! (`brain::turn`, `chat::send`, ...), não para este arquivo.
//!
//! Convertidos aqui: failover de provedor pra Ollama (`brain/turn.rs`), provedor desativado
//! (`chat/send.rs`, `vision/watcher.rs`), boot degradado (`feature_health.rs`), mudo de fala
//! (`voice/speak_tool.rs`, `shortcuts.rs`) — os momentos que specs/10-observability.md nomeia.
//! Fallback de TTS (ElevenLabs -> Supertonic) não existe no Rust hoje (ver o cabeçalho de
//! `voice/tts/dispatch.rs`: "reportado, não implementado"), então não há call site pra converter.

/// `human_log!(Level::Warn, "caí para o Ollama local porque {reason}")` — mesma sintaxe de
/// `log::log!`. Use só `Level::Info`/`Warn`/`Error`: um `Debug`/`Trace` aqui cairia no tier `ai`
/// por nível (`audience_for`) e a frase-com-efeito perderia o leitor a que se destina.
#[macro_export]
macro_rules! human_log {
    ($level:expr, $($arg:tt)+) => {
        log::log!($level, $($arg)+)
    };
}
