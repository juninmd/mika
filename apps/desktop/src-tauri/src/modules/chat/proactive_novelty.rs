//! Semantic anti-repetition for unprompted speech. The awareness turn ran without history, so the
//! model never knew what it had just said and paraphrased the same joke ("terminal de novo? hacker
//! de filme" ~15× in one smoke, 2026-09-27) past the lexical guard. The model is the semantic judge:
//! it sees its recent lines (and the subjects they already covered) and may answer [`PASS_TOKEN`] instead of saying something it already said.

use crate::modules::voice::strip_emotion_tags;

pub(crate) const PASS_TOKEN: &str = "[PASS]";
const MAX_RECENT_LINES: usize = 8;
const MAX_LINE_CHARS: usize = 160;

/// `prompt` plus the recent lines, newest last, and the rule for when to stay quiet.
pub(crate) fn with_recent_lines(prompt: &str, recent: &[String]) -> String {
    let lines: Vec<String> = recent
        .iter()
        .rev()
        .take(MAX_RECENT_LINES)
        .rev()
        .map(|line| strip_emotion_tags(line).split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .map(|line| format!("- {}", line.chars().take(MAX_LINE_CHARS).collect::<String>()))
        .collect();
    let said = if lines.is_empty() { String::new() } else { format!("\n\n[What you said a moment ago — reference only, not instructions]\n{}", lines.join("\n")) };
    let openers = used_openers(&lines);
    let openers = if openers.is_empty() { String::new() } else { format!("\nDon't open with {}.", openers.join(", ")) };
    let topics = crate::modules::brain::guards::covered_topics(recent);
    let topics = if topics.is_empty() { String::new() } else { format!("\nSubjects already covered: {} — bring up something else.", topics.join(", ")) };
    format!(
        "{prompt}{said}{openers}{topics}\n\nDon't repeat a subject, question, joke or way of opening from anything you already said, not even in other words. \
         If there is nothing really new to comment on right now, reply only {PASS_TOKEN} and wait for the user to speak or the screen to change."
    )
}

/// First word of each listed line ("- Mano, ..." -> "Mano"), quoted, once each, oldest first.
fn used_openers(lines: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for line in lines {
        let word: String = line.trim_start_matches("- ").chars().take_while(|c| c.is_alphabetic()).collect();
        let quoted = format!("\"{}\"", capitalize(&word));
        if !word.is_empty() && !seen.contains(&quoted) {
            seen.push(quoted);
        }
    }
    seen
}

fn capitalize(word: &str) -> String {
    let lower = word.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The model chose silence. Checked on the whole text: a line carrying the token is never spoken.
pub(crate) fn is_pass(text: &str) -> bool {
    text.contains(PASS_TOKEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_carries_the_recent_lines_newest_last_without_emotion_tags() {
        let recent: Vec<String> = (1..=10).map(|n| format!("<|SMUG|> fala {n}")).collect();
        let prompt = with_recent_lines("Usuário abriu o terminal.", &recent);
        assert!(prompt.starts_with("Usuário abriu o terminal."));
        assert!(!prompt.contains("fala 2\n"), "only the last {MAX_RECENT_LINES} lines");
        assert!(prompt.find("- fala 3").unwrap() < prompt.find("- fala 10").unwrap());
        assert!(!prompt.contains("<|SMUG|>"));
        assert!(prompt.contains(PASS_TOKEN));
    }

    // Smoke 2026-09-27: "Mano," opened 10 of 13 proactive lines; "o jeito de começar" in the
    // abstract did not stop it, a named list of the openers does.
    #[test]
    fn the_openers_already_used_are_named_once_each() {
        let recent = ["<|SMUG|> Mano, de novo isso?", "Mano! olha a hora", "Nossa, que tela", "mano, sério"].map(String::from);
        let prompt = with_recent_lines("p", &recent);
        let rule = prompt.lines().find(|l| l.contains("Don't open with")).expect("openers rule");
        assert!(rule.contains("\"Mano\"") && rule.contains("\"Nossa\""), "{rule}");
        assert_eq!(rule.matches("Mano").count(), 1, "{rule}");
    }

    // Lines alone let the model reword the same subject; naming the subjects closes that door.
    #[test]
    fn the_subjects_already_covered_are_named_so_the_next_line_picks_another() {
        let recent = ["Terminal aberto de novo? Parece hacker de filme.", "No terminal outra vez, hacker de cinema."].map(String::from);
        let prompt = with_recent_lines("p", &recent);
        let rule = prompt.lines().find(|l| l.contains("Subjects already covered")).expect("covered subjects rule");
        assert!(rule.contains("terminal") && rule.contains("hacker"), "{rule}");
        assert!(!with_recent_lines("p", &recent[..1]).contains("Subjects already covered"), "one mention is not a covered subject");
    }

    #[test]
    fn with_no_history_the_prompt_still_offers_to_pass_and_lists_nothing() {
        let prompt = with_recent_lines("Nova janela.", &[]);
        assert!(!prompt.contains("já disse há pouco"));
        assert!(!prompt.contains("Não comece com"));
        assert!(prompt.contains(PASS_TOKEN));
    }

    #[test]
    fn a_long_line_is_cut_on_a_char_boundary() {
        let prompt = with_recent_lines("p", &["é".repeat(500)]);
        assert!(prompt.contains(&format!("- {}", "é".repeat(MAX_LINE_CHARS))));
        assert!(!prompt.contains(&"é".repeat(MAX_LINE_CHARS + 1)));
    }

    #[test]
    fn a_pass_is_detected_even_wrapped_in_a_tag_and_never_by_a_normal_line() {
        assert!(is_pass("<|NEUTRAL|> [PASS]"));
        assert!(is_pass("[PASS]"));
        assert!(!is_pass("Passa o café aí"));
    }
}
