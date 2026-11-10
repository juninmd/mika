//! Feature `computer_use` — WP5, "computer use" (agent-harness defect P0 in `defects.md`: no
//! input-simulation tool existed). Tool `computer`: `find`/`click_element` (Windows, via UI
//! Automation) and `click`/`type_text`/`key` (every OS, via `enigo`), so a chat request like
//! "abre a calculadora e clica no 7" can actually move the mouse and type.
//!
//! One directory (not a single file) for the same reason `features::desktop` is one: each concern
//! is independently testable —
//! - [`key_combo`]: parses `"ctrl+c"`/`"enter"` into `enigo::Key`s. Pure.
//! - [`input_backend`]: the trait that actually moves the mouse/keyboard (`EnigoBackend`) plus a
//!   recording fake (`#[cfg(test)] pub(crate) mod fake`) so nothing in this feature's tests ever
//!   touches the real cursor.
//! - [`foreground_guard`]: refuses `click`/`type_text`/`key` when the CURRENT foreground window is
//!   this process (Mika clicking herself) — Windows-only probe, same cut as
//!   `system_signals::active_window`.
//! - [`ui_automation`]: `find`/`click_element` (Windows via the `uiautomation` crate; every other
//!   OS gets a named `"unsupported on this platform"` result) plus the password-field refusal
//!   (`UIA IsPassword`) `type_text` also consults.
//! - [`tool`]: the `computer` tool itself — argument validation, action routing, the
//!   `computerUse.enabled` config gate — wired to `ToolRegistry::register_destructive` so every
//!   call goes through the approval broker (see `tool.rs`'s header for why that choice, not
//!   `desktop`'s plain `register`).
//! - [`settings`]: the `computer-use` settings tab (`ComputerUsePanel`,
//!   `apps/desktop/src/features/computer-use/renderer/settings.tsx`).
//!
//! **One registration pass, not two.** Unlike `mood`/`desktop`/`person_catalog`, this tool needs
//! `ConfigStore` (the `computerUse.enabled` switch) from the moment it exists — there is no
//! config-free capability worth exposing before that. So [`register`] is called ONLY from
//! `modules::register_config_bound_tools` (the second pass, with `AppHandle`); there is no
//! `register_tools()`/first-pass stub, the same shape `vision`/`user_profile`/`reminder` already
//! use.

mod foreground_guard;
mod input_backend;
mod key_combo;
mod settings;
mod tool;
mod ui_automation;

pub(crate) use settings::register;
