//! Porte de `splitIntoSentences` — separado de `preprocessor.rs` só por tamanho (AGENTS.md, teto de
//! 300 linhas), mesma lógica pura de sempre. `SENTENCE_SPLIT_MIN_LENGTH` mirrors `constants.ts` (60).

/// Mesmo espírito de `voice::auto_speak::stream_sentences::ABBREVIATIONS` (duplicada de propósito:
/// os dois cortadores de frase moram em módulos que não se importam um do outro — mesmo precedente
/// aceito no repo para uma leitura pequena atravessar um limite de módulo, ver o cabeçalho de
/// `brain/turn.rs`). Este serve o caminho de fala JÁ FECHADA (`speaker/mod.rs`); o outro serve o
/// streaming. `p.ex` tem ponto embutido de propósito — ver [`word_before`].
const ABBREVIATIONS: &[&str] =
    &["sr", "sra", "srta", "dr", "dra", "prof", "profa", "eng", "exmo", "exma", "av", "vs", "jr", "st", "p.ex"];

pub const SENTENCE_SPLIT_MIN_LENGTH: usize = 60;

/// A palavra (delimitada por espaço) imediatamente antes de `dot_index`, sem incluir o ponto —
/// `dot_index` sempre aponta para um `.` ASCII, então nunca corta um char multibyte ao fatiar.
fn word_before(text: &str, dot_index: usize) -> &str {
    let start = text[..dot_index].char_indices().rev().find(|&(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8());
    &text[start..dot_index]
}

fn ends_in_abbreviation(text: &str, dot_index: usize) -> bool {
    ABBREVIATIONS.contains(&word_before(text, dot_index).to_lowercase().as_str())
}

/// Equivalente a `text.split(/(?<=[.!?…])\s+/)` do TS. O crate `regex` não suporta lookbehind
/// (`(?<=...)`) — em vez de simular com uma segunda passada, isto varre a string uma vez e corta
/// logo depois de um caractere terminal seguido de espaço, mantendo a pontuação no pedaço anterior
/// (o mesmo efeito observável do lookbehind, sem herdar a dependência). Um `.` só fecha frase se a
/// palavra antes dele não for abreviação — sem isso "Dr." virava um chunk sozinho, e um número como
/// "3.5" nunca chega aqui de qualquer forma (só o `.` seguido de ESPAÇO é candidato; "3.5" não tem
/// espaço nenhum entre os dígitos).
fn split_after_terminal_punctuation(text: &str) -> Vec<&str> {
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        let (byte_idx, c) = bytes[i];
        let next_is_whitespace = bytes.get(i + 1).is_some_and(|&(_, next)| next.is_whitespace());
        if matches!(c, '。' | '！' | '？') {
            // CJK não tem espaço depois do ponto; os fechadores (」』）) ficam na mesma frase.
            let mut end_i = i + 1;
            while bytes.get(end_i).is_some_and(|&(_, next)| is_cjk_terminal_or_closer(next)) {
                end_i += 1;
            }
            let end = bytes.get(end_i).map_or(text.len(), |&(idx, _)| idx);
            parts.push(&text[start..end]);
            start = end;
            i = end_i;
            continue;
        }
        if matches!(c, '.' | '!' | '?' | '…') && next_is_whitespace {
            if c == '.' && ends_in_abbreviation(text, byte_idx) {
                i += 1;
                continue;
            }
            let end = bytes[i + 1].0;
            parts.push(&text[start..end]);

            let mut j = i + 1;
            while j < bytes.len() && bytes[j].1.is_whitespace() {
                j += 1;
            }
            start = bytes.get(j).map_or(text.len(), |&(idx, _)| idx);
            i = j;
            continue;
        }
        i += 1;
    }
    parts.push(&text[start..]);
    parts
}

fn is_cjk_terminal_or_closer(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '」' | '』' | '）')
}

pub fn split_into_sentences(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for part in split_after_terminal_punctuation(text) {
        current = if current.is_empty() { part.to_string() } else { format!("{current} {part}") };

        let ends_terminal = current.chars().last().is_some_and(|c| matches!(c, '.' | '!' | '?' | '…') || is_cjk_terminal_or_closer(c));
        if current.chars().count() >= SENTENCE_SPLIT_MIN_LENGTH || ends_terminal {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                chunks.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        chunks.push(trimmed);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_into_sentences_breaks_on_terminal_punctuation() {
        let out = split_into_sentences("Oi! Tudo bem? Sim.");
        assert_eq!(out, vec!["Oi!", "Tudo bem?", "Sim."]);
    }

    #[test]
    fn split_into_sentences_accumulates_short_fragments_until_the_minimum_length() {
        let long_first = "a".repeat(70);
        let out = split_into_sentences(&format!("{long_first}. resto"));
        assert_eq!(out[0], format!("{long_first}."));
    }

    #[test]
    fn split_into_sentences_breaks_on_cjk_terminals_without_spaces() {
        assert_eq!(split_into_sentences("こんにちは。元気ですか？はい！"), vec!["こんにちは。", "元気ですか？", "はい！"]);
        assert_eq!(split_into_sentences("你好。今天天气很好！"), vec!["你好。", "今天天气很好！"]);
        assert_eq!(split_into_sentences("彼は「行こう。」と言った。"), vec!["彼は「行こう。」", "と言った。"]);
    }

    #[test]
    fn split_into_sentences_never_returns_empty_chunks() {
        assert_eq!(split_into_sentences("   "), Vec::<String>::new());
    }

    #[test]
    fn split_into_sentences_does_not_break_after_an_abbreviation() {
        let out = split_into_sentences("Cheguei e falei com o Dr. Silva sobre o caso.");
        assert_eq!(out, vec!["Cheguei e falei com o Dr. Silva sobre o caso."], "\"Dr.\" sozinho não pode virar um chunk");
    }

    #[test]
    fn split_into_sentences_recognizes_sr_sra_and_pex_as_abbreviations() {
        for abbrev in ["Sr.", "Sra.", "p.ex."] {
            let text = format!("Chame o {abbrev} responsável e resolva isso.");
            assert_eq!(split_into_sentences(&text), vec![text.clone()], "{abbrev} não pode fechar frase sozinho");
        }
    }

    #[test]
    fn split_into_sentences_never_cuts_a_decimal_number() {
        let out = split_into_sentences("A versão 3.5 já resolveu isso ontem.");
        assert_eq!(out, vec!["A versão 3.5 já resolveu isso ontem."]);
    }
}
