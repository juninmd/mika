//! Drops a paragraph where the model narrates its own reasoning ("O usuário quer…, rodei o
//! `get_system_info`…") before the reply. Gemma did it in 6 of 6 tool turns once `<|THINKING|>` was
//! allowed as the opening tag, and each leaked turn went back into the history the model imitates
//! (`smoke-baseline.md` item 3).
//!
//! Detection is narrow and tag-gated, never content-only: a paragraph is narration ONLY when it
//! opens with an emotion tag (`<|WORD|>`) immediately followed by either the model's own explicit
//! `[thinking]` marker or the pt-BR narration idiom "O usuário ..." (third person, describing the
//! user instead of speaking to them). A tag followed by direct speech ("Hmm, deixa eu ver...") is
//! never touched, and a paragraph with no tag at all is never a candidate — real replies don't open
//! with one, so nothing legitimate can collide with this heuristic.
//!
//! One instance covers one model generation: `turn.rs` builds a fresh `NarrationFilter` right before
//! each step's stream and calls `finish()` right after it ends, so "narration after a tool call" is
//! just the next step's own leading paragraph, caught the same way — this filter has no idea tools
//! exist.
//!
//! Streaming contract: `push` holds back only the still-undecided START of the CURRENT paragraph
//! (bounded by [`MAX_PROBE_BYTES`]) — once a paragraph is decided, every further character streams
//! out (or is dropped) immediately, so a kept reply is never delayed by more than one short probe. A
//! reply that turns out to be ENTIRELY narration is still spoken in full (`finish`), never silenced,
//! because dropping it would leave nothing at all.

const THINKING_MARKER: &str = "[thinking]";
const NARRATION_PHRASE: &str = "o usuário";
/// Bytes of a paragraph's start held before a decision is forced. Long enough to fit a tag plus
/// either signal in full; short enough that no real reply is meaningfully delayed.
const MAX_PROBE_BYTES: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Decision {
    Keep,
    Drop,
}

pub(super) struct NarrationFilter {
    /// Full input, verbatim — the only way to recover a reply that turns out to be 100% narration.
    raw: String,
    /// Undecided head of the CURRENT paragraph; empty once a decision has been made for it.
    probe: String,
    decision: Option<Decision>,
    /// Separator (run of `\n`) confirmed to precede the CURRENT paragraph, held until its decision
    /// is known — flushed only if the paragraph is kept AND something was already emitted (an
    /// all-narration reply must not start with a stray blank line).
    held_separator: String,
    /// Newlines seen since the last non-newline character, not yet confirmed as a paragraph
    /// boundary — a single one may still turn out to be a soft line break inside the paragraph.
    scanning_newlines: String,
    /// Whether anything has ever reached `push`'s or `finish`'s output — gates the leading
    /// separator above, and (in `finish`) whether the "only narration" fallback applies.
    emitted_any: bool,
}

impl NarrationFilter {
    pub(super) fn new() -> Self {
        Self {
            raw: String::new(),
            probe: String::new(),
            decision: None,
            held_separator: String::new(),
            scanning_newlines: String::new(),
            emitted_any: false,
        }
    }

    /// Text safe to show and speak now; the start of a paragraph is held until it can decide.
    pub(super) fn push(&mut self, chunk: &str) -> String {
        self.raw.push_str(chunk);
        let mut out = String::new();
        for c in chunk.chars() {
            self.feed(c, &mut out);
        }
        out
    }

    /// Flushes whatever the last paragraph still owes (forcing its decision if never resolved) and,
    /// if the whole reply was narration end to end, recovers the original text instead of silence.
    pub(super) fn finish(&mut self) -> String {
        let mut out = String::new();
        if self.decision.is_none() {
            let decision = classify(&self.probe, true);
            self.resolve(decision, &mut out);
        }
        if self.decision == Some(Decision::Keep) {
            out.push_str(&self.scanning_newlines);
        }
        self.scanning_newlines.clear();
        if !self.emitted_any {
            return std::mem::take(&mut self.raw);
        }
        out
    }

    /// One character of input. Newlines are held back one at a time — only a confirmed run of 2+
    /// becomes a paragraph boundary; a lone `\n` turns out to be ordinary content once whatever
    /// follows it isn't another `\n` (see [`Self::start_new_paragraph`]).
    fn feed(&mut self, c: char, out: &mut String) {
        if c == '\n' {
            self.scanning_newlines.push('\n');
            return;
        }
        match self.scanning_newlines.len() {
            0 => {}
            1 => {
                self.scanning_newlines.clear();
                self.feed_paragraph_char('\n', out);
            }
            _ => self.start_new_paragraph(out),
        }
        self.feed_paragraph_char(c, out);
    }

    fn feed_paragraph_char(&mut self, c: char, out: &mut String) {
        match self.decision {
            Some(Decision::Keep) => out.push(c),
            Some(Decision::Drop) => {}
            None => {
                self.probe.push(c);
                let force = self.probe.len() >= MAX_PROBE_BYTES;
                let decision = classify(&self.probe, force);
                self.resolve(decision, out);
            }
        }
    }

    /// A confirmed blank line: closes out whatever paragraph was in progress (forcing its decision
    /// if it never resolved on its own — e.g. a bare tag with nothing after it) and hands the just
    /// confirmed separator to the paragraph about to start.
    fn start_new_paragraph(&mut self, out: &mut String) {
        if self.decision.is_none() {
            let decision = classify(&self.probe, true);
            self.resolve(decision, out);
        }
        self.held_separator = std::mem::take(&mut self.scanning_newlines);
        self.decision = None;
    }

    /// Applies a just-made decision: `Keep` flushes the held separator (only if something was
    /// already emitted) then the probe; `Drop` discards both silently.
    fn resolve(&mut self, decision: Option<Decision>, out: &mut String) {
        let Some(decision) = decision else { return };
        self.decision = Some(decision);
        if decision == Decision::Keep {
            if self.emitted_any {
                out.push_str(&self.held_separator);
            }
            out.push_str(&self.probe);
            self.emitted_any = true;
        }
        self.probe.clear();
        self.held_separator.clear();
    }
}

/// Classifies a paragraph from its still-growing start. `force` (probe budget spent, or end of
/// input) resolves every remaining ambiguity as `Keep` — never silence real speech on a guess.
fn classify(probe: &str, force: bool) -> Option<Decision> {
    if !probe.starts_with("<|") {
        return if probe.len() < 2 && !force { None } else { Some(Decision::Keep) };
    }
    let Some(rel_close) = probe[2..].find("|>") else {
        return if force { Some(Decision::Keep) } else { None };
    };
    let after = probe[2 + rel_close + 2..].trim_start();
    if after.is_empty() {
        return if force { Some(Decision::Keep) } else { None };
    }
    let after_lower = after.to_lowercase();
    if after_lower.starts_with(THINKING_MARKER) || after_lower.starts_with(NARRATION_PHRASE) {
        return Some(Decision::Drop);
    }
    let ambiguous = THINKING_MARKER.starts_with(after_lower.as_str()) || NARRATION_PHRASE.starts_with(after_lower.as_str());
    if ambiguous && !force { None } else { Some(Decision::Keep) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(chunks: &[&str]) -> String {
        let mut filter = NarrationFilter::new();
        let mut out: String = chunks.iter().map(|chunk| filter.push(chunk)).collect();
        out.push_str(&filter.finish());
        out
    }

    #[test]
    fn a_reasoning_paragraph_is_dropped_before_the_reply() {
        let leaked = "<|THINKING|> [THINKING] O usuário quer um diagnóstico do PC. Rodei o `get_system_info`.\n\n<|SMUG|> Mano, tá liso!";
        assert_eq!(run(&[leaked]), "<|SMUG|> Mano, tá liso!");
    }

    #[test]
    fn the_narration_is_caught_even_when_split_across_chunks() {
        let chunks = ["<|THIN", "KING|> [THINKING] O usu", "ário tá insistindo. Vou rodar", " de novo.\n", "\n<|LAUGH|> Kkkk, ", "de novo?"];
        assert_eq!(run(&chunks), "<|LAUGH|> Kkkk, de novo?");
    }

    #[test]
    fn a_thinking_face_that_speaks_to_the_user_is_kept() {
        let reply = "<|THINKING|> Hmm, deixa eu ver o que você tem aí.\n\nO usuário do Discord te respondeu?";
        assert_eq!(run(&["<|THINKING|> Hmm, deixa eu ver", " o que você tem aí.\n\nO usuário do Discord te respondeu?"]), reply);
    }

    #[test]
    fn a_reply_that_is_only_narration_is_spoken_rather_than_silenced() {
        let only = "<|THINKING|> O usuário quer saber a memória livre, 8 GB.";
        assert_eq!(run(&[only]), only);
    }

    #[test]
    fn plain_text_passes_unchanged() {
        assert_eq!(run(&["Oi", "!"]), "Oi!");
        assert_eq!(run(&["<|SMUG|> Olha só.\n\n", "<|LAUGH|> Kkk."]), "<|SMUG|> Olha só.\n\n<|LAUGH|> Kkk.");
    }

    /// Pins the exact production defect (`smoke-baseline.md` item 3): lowercase `[thinking]`, an
    /// ellipsis-heavy reasoning paragraph, real content following.
    #[test]
    fn the_exact_smoke_baseline_leak_is_dropped() {
        let leaked = "<|THINKING|> [thinking] O usuário perguntou sobre o jogo... Vou responder de forma natural...\n\n<|SMUG|> Bora jogar!";
        assert_eq!(run(&[leaked]), "<|SMUG|> Bora jogar!");
    }

    #[test]
    fn repeated_narration_paragraphs_are_all_dropped() {
        let leaked = "<|THINKING|> [thinking] O usuário quer ajuda.\n\n<|THINKING|> [thinking] O usuário insistiu de novo.\n\n<|LAUGH|> Beleza, bora!";
        assert_eq!(run(&[leaked]), "<|LAUGH|> Beleza, bora!");
    }

    /// `turn.rs` builds a fresh filter per step, so "narration after a tool call" is really just a
    /// later step's own leading paragraph (the model reasoning about the tool result it just got
    /// back) — a NEW instance must catch it exactly like the very first step does.
    #[test]
    fn a_fresh_instance_still_catches_narration_that_follows_a_tool_result() {
        let after_tool_result =
            "<|THINKING|> [thinking] O usuário quer que eu clique no botão. Rodei a tool desktop e não existe ação de clique.\n\n<|LAUGH|> Deu ruim, ainda não sei clicar em nada!";
        assert_eq!(run(&[after_tool_result]), "<|LAUGH|> Deu ruim, ainda não sei clicar em nada!");
    }
}
