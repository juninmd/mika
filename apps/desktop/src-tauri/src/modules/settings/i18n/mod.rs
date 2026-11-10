//! Resolução de i18n para declarações de settings — pré-requisito duro de `mika:settings-
//! declarations` (ver o cabeçalho de `feature_registry/mod.rs`, seção "i18n fica de fora, de
//! propósito"): o registro guarda `Declaration` crua (`settings.voice.title`), e é este módulo
//! quem resolve para texto antes de responder, espelhando `ipc/settings.ts::registerSettingsIPC`
//! (`featureRegistry.getAllSettings()` → `localizeFeatureSettings(..., currentAppLocale())`).
//!
//! Três arquivos porque cada um tem prova própria: `parser.rs` (como ler um `TranslationDict` TS
//! sem um parser TS completo), `locales.rs` (as 10 traduções embutidas + `t()` com fallback),
//! `localize.rs` (aplica `t()` numa `Declaration` inteira, campo a campo).
//!
//! **Ainda não ligado a nenhum canal.** `mika:settings-declarations` continua no sidecar — o
//! catálogo do `FeatureRegistry` ainda não está populado por todos os módulos que a UI espera (ver
//! o cabeçalho de `settings/mod.rs`). `t`/`localize_feature_settings` ficam prontos e testados
//! para quem completar essa população e reivindicar o canal.

mod locales;
mod localize;
mod parser;

pub use locales::{t, tf};
pub use localize::localize_feature_settings;
