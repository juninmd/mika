//! Cuts a step whose stream degenerated into emotion tags only. Smoke 2026-09-26: Gemma emitted
//! `<|THINKING|>` hundreds of times for 30 s — nothing speakable, so no barge-in could cut it, and
//! the user's next turn queued behind it.

use std::sync::LazyLock;

use regex::Regex;

/// Consecutive tags with nothing but whitespace between them. A real reply carries one or two.
const MAX_CONSECUTIVE_TAGS: usize = 8;
/// Tail kept for the check: the run we look for sits at the end, so older text is irrelevant.
const TAIL_CHARS: usize = 512;

static TRAILING_TAG_RUN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?:<\|[A-Za-z_0-9]+(?::[\d.]+)?\|>\s*){{{MAX_CONSECUTIVE_TAGS},}}$")).expect("tag-run pattern")
});

#[derive(Default)]
pub(super) struct TagLoopGuard {
    tail: String,
}

impl TagLoopGuard {
    /// Feeds one raw stream fragment; `true` once the stream ends in a degenerate run of tags.
    pub(super) fn push(&mut self, fragment: &str) -> bool {
        self.tail.push_str(fragment);
        let excess = self.tail.chars().count().saturating_sub(TAIL_CHARS);
        if excess > 0 {
            self.tail = self.tail.chars().skip(excess).collect();
        }
        TRAILING_TAG_RUN.is_match(self.tail.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(chunks: &[&str]) -> bool {
        let mut guard = TagLoopGuard::default();
        chunks.iter().any(|chunk| guard.push(chunk))
    }

    #[test]
    fn a_stream_of_repeated_tags_split_across_chunks_is_cut() {
        assert!(feed(&["<|THINKING|><|THIN", "KING|> <|THINKING|>", &"<|THINKING|> ".repeat(6)]));
    }

    #[test]
    fn a_run_after_real_text_is_still_cut() {
        assert!(feed(&["Mano, o motor tem quatro tempos. ", &"<|THINKING|>".repeat(9)]));
    }

    #[test]
    fn normal_replies_with_a_few_tags_pass() {
        assert!(!feed(&["<|THINKING|><|THINKING|>\n\n<|SITTING|>Mano, ", "agora você quer virar mecânico? <|SMUG|> Tá bom."]));
    }

    #[test]
    fn tags_separated_by_words_never_count_as_a_run() {
        assert!(!feed(&[&"<|SMUG|> sim ".repeat(20)]));
    }
}
