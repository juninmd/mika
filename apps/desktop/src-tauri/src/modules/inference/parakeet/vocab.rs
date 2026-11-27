//! `vocab.txt` of the export: one `<token> <id>` per line. `▁` marks a word start, exactly as the
//! onnx-asr decoder consumes it. Blank is the `<blk>` piece, not a hardcoded id.

use std::sync::OnceLock;

use regex::Regex;

pub struct Vocab {
    pieces: Vec<String>,
    blank: usize,
}

fn space_pattern() -> &'static Regex {
    // Same pattern as onnx-asr's `_AsrWithDecoding.DECODE_SPACE_PATTERN`: drop the leading space and
    // the space before punctuation, keep the one before a word. The capture group is re-inserted.
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\A\s|\s\B|(\s)\b").expect("regex de detokenização válido"))
}

impl Vocab {
    pub fn parse(text: &str) -> Self {
        let mut pieces = Vec::new();
        let mut blank = 0usize;
        for line in text.lines() {
            let Some((piece, id)) = line.rsplit_once(' ') else { continue };
            let Ok(id) = id.trim().parse::<usize>() else { continue };
            if id >= pieces.len() {
                pieces.resize(id + 1, String::new());
            }
            pieces[id] = piece.to_string();
            if piece == "<blk>" {
                blank = id;
            }
        }
        Self { pieces, blank }
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn blank(&self) -> usize {
        self.blank
    }

    /// A truncated `vocab.txt` may end before `<blk>`; downloading then decoding would silently use
    /// whatever id happens to sit at index 0 as blank.
    pub fn has_blank(&self) -> bool {
        self.pieces.get(self.blank).is_some_and(|piece| piece == "<blk>")
    }

    pub fn decode(&self, ids: &[usize]) -> String {
        let joined: String = ids
            .iter()
            .filter_map(|&id| self.pieces.get(id))
            .map(String::as_str)
            .collect::<String>()
            .replace('\u{2581}', " ");
        space_pattern().replace_all(&joined, "$1").trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vocab {
        Vocab::parse("<blk> 0\n\u{2581}Ah 1\n, 2\n\u{2581}ela 3\n\u{2581}voltou 4\n? 5")
    }

    #[test]
    fn joins_pieces_on_word_marks_and_keeps_blank_explicit() {
        let vocab = sample();
        assert_eq!(vocab.len(), 6);
        assert_eq!(vocab.blank(), 0);
        assert_eq!(vocab.decode(&[1, 3, 4]), "Ah ela voltou");
    }

    #[test]
    fn drops_the_space_before_punctuation_but_not_before_words() {
        assert_eq!(sample().decode(&[1, 2, 3, 4, 5]), "Ah, ela voltou?");
    }

    #[test]
    fn out_of_range_ids_are_skipped_not_panicking() {
        assert_eq!(sample().decode(&[99, 1]), "Ah");
    }
}
