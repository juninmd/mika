//! Bloco "rotina" do turno: o corpo da Mika (sono/sede/fome/tédio, `features::mood::needs`) e os
//! hábitos do usuário (top programas 7d, top jogos 30d, `features::desktop::habits`). A hora/data/
//! fuso já vêm em "## TIME AWARENESS" (`system_prompt.rs`), não repetidas aqui.
//!
//! Orçamento ~60 tokens: o corpo só entra quando alguma necessidade passa de
//! `needs::NOTICEABLE`; hábitos só com alguma lista não vazia. Texto em
//! `prompts/brain/body-state.md` e `prompts/brain/user-habits.md` — aqui só os valores:
//! - `body-state.md`: `{{needs}}` = [`prompt_line`] (`sleepy (strong), thirsty (mild)`).
//! - `user-habits.md`: `{{programs}}`/`{{games}}` = [`compact_list`] (`Code 12h, chrome 45min`) ou
//!   `none yet`.

use std::collections::HashMap;

use crate::config::store::ConfigStore;
use crate::modules::chat::load_prompt_no_handle;
use crate::modules::features::desktop::{habits, usage::compact_list};
use crate::modules::features::mood::needs::{prompt_line, runtime::current_needs};

fn render(key: &str, vars: &[(&str, String)]) -> Option<String> {
    let map: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    load_prompt_no_handle("brain", key, &map)
        .inspect_err(|error| log::error!("persona: prompt brain/{key} não carregou: {error}"))
        .ok()
        .map(|text| text.trim().to_string())
}

/// Linhas prontas → texto do bloco. Pura sobre os valores, para o teste não depender do relógio.
fn format_routine(needs_line: &str, programs: &str, games: &str) -> String {
    let mut parts = Vec::new();
    if !needs_line.is_empty() {
        parts.extend(render("body-state", &[("needs", needs_line.to_string())]));
    }
    if !programs.is_empty() || !games.is_empty() {
        let or_none = |list: &str| if list.is_empty() { "none yet".to_string() } else { list.to_string() };
        parts.extend(render("user-habits", &[("programs", or_none(programs)), ("games", or_none(games))]));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("\n\n{}\n", parts.join("\n\n"))
}

pub(super) fn build_routine_block(store: &ConfigStore) -> String {
    let needs_line = current_needs(store).map(|n| prompt_line(&n)).unwrap_or_default();
    let programs = compact_list(&habits::top_programs(store, 7));
    let games = compact_list(&habits::top_games(store));
    format_routine(&needs_line, &programs, &games)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_felt_and_no_habits_adds_nothing_to_the_prompt() {
        assert_eq!(format_routine("", "", ""), "");
    }

    #[test]
    fn the_body_line_and_the_habits_come_from_the_prompt_files() {
        let block = format_routine("sleepy (strong)", "Code 12h, chrome 3h", "");
        assert!(block.contains("## Your body\nYou feel sleepy (strong)."), "{block}");
        assert!(block.contains("Top programs (7d): Code 12h, chrome 3h. Top games (30d): none yet."), "{block}");
        assert!(!block.contains("{{"), "placeholder cru: {block}");
    }

    /// Orçamento: corpo + cinco programas + cinco jogos cabem em ~70 palavras (≈ tokens).
    #[test]
    fn a_full_block_stays_within_the_budget() {
        let five = "Code 12h, chrome 8h, Discord 3h, Spotify 2h, obs64 1h";
        let block = format_routine("sleepy (strong), thirsty (mild)", five, "Elden Ring 20h, Hades 6h, Balatro 4h, Dota 2 3h, Celeste 1h");
        let words = block.split_whitespace().count();
        assert!(words <= 70, "{words} palavras:\n{block}");
    }
}
