//! Feature `scheduler` em Rust — port PARCIAL da tool `reminder`
//! (`packages/scheduler/src/tools/reminder.tool.ts` + `services/scheduler.service.ts` +
//! `adapters/sqlite-reminder.repository.ts`, 785 LOC lidas nesta rodada).
//!
//! ## Tabela TS × Rust
//!
//! | Peça | Depende de | Rust hoje |
//! |---|---|---|
//! | `SqliteReminderRepository` (CRUD) | só SQLite (`rusqlite`, já dependência) | **portado** ([`store`]) |
//! | `action=schedule` (amount+unit) | só relógio do sistema | **portado** ([`tool`]) |
//! | `action=list`/`action=cancel` | só o repositório | **portado** |
//! | `action=schedule` (atTime, horário local) | fuso + DST (`local-time.ts`) | **portado** (fuso do SO via `chrono::Local`) |
//! | Disparo automático (`SchedulerService.tick`, a cada 20s) | `tokio::spawn` + notificação/fala | **portado** ([`firing`]) |
//!
//! `atTime` usa o fuso do SO (`chrono::Local`: API do Windows, `/etc/localtime` no Unix) como banco
//! de fusos, sem `chrono-tz`. Hora inexistente
//! num buraco de DST cai na hora seguinte.
//!
//! Divergência do disparo: o lembrete sai do banco ANTES da fala (a notificação já o entregou), em
//! vez do teto de 3 tentativas do TS — ver o cabeçalho de [`firing`].
pub mod feature;
mod firing;
mod plugin_test;
mod store;
mod tool;

pub use tool::register;
/// Reexport estreito para `features::privacy` reusar o MESMO banco de lembretes em
/// `clear_history`/`export_data` (limpar/exportar) em vez de reabrir um caminho próprio.
pub(crate) use store::{db_path, SchedulerStore};
