//! `vocab.txt` of the export: one SentencePiece piece per line, line number = token id. `▁` marks
//! a word start; `<xx-XX>` pieces are the language tag the model emits and never reach the text.

pub struct Vocab {
    pieces: Vec<String>,
}

impl Vocab {
    pub fn parse(text: &str) -> Self {
        Self { pieces: text.lines().map(str::to_string).collect() }
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn decode(&self, ids: &[usize]) -> String {
        let text: String = ids
            .iter()
            .filter_map(|&id| self.pieces.get(id))
            .filter(|piece| !(piece.starts_with('<') && piece.ends_with('>')))
            .map(String::as_str)
            .collect();
        text.replace('\u{2581}', " ").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_pieces_on_word_marks_and_drops_tags() {
        let vocab = Vocab::parse("<unk>\n<pt-BR>\n\u{2581}Oi\n\u{2581}Mi\nka\n,\n<blank>");
        assert_eq!(vocab.decode(&[2, 3, 4, 5, 1]), "Oi Mika,");
    }

    #[test]
    fn out_of_range_ids_are_skipped_not_panicking() {
        assert_eq!(Vocab::parse("\u{2581}a").decode(&[0, 99]), "a");
    }
}
