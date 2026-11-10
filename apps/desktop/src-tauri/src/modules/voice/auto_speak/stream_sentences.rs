//! Porte fiel de `apps/desktop/src/main/services/voice/audio/stream-sentences.ts` — o recorte de
//! frases que faz a Mika começar a falar ANTES do LLM terminar de gerar. Puro de propósito: nenhum
//! `Regex` fica `static`/`OnceLock` de processo, porque o crate `regex` não tem estado tipo
//! `lastIndex` do JS para vazar entre chamadas (o teste do TS que prova isso — "a regex de fronteira
//! é global e reutilizada" — não tem equivalente necessário aqui).

use std::sync::LazyLock;

use regex::Regex;

const MIN_CHUNK_CHARS: usize = 24;
const MAX_CHUNK_CHARS: usize = 180;

/// Abreviatura termina em ponto e NÃO fecha frase. `etc.`/`obs.` ficam de fora de propósito: quase
/// sempre fecham a frase de verdade — mesma lista do TS.
const ABBREVIATIONS: &[&str] =
    &["sr", "sra", "srta", "dr", "dra", "prof", "profa", "eng", "exmo", "exma", "av", "vs", "jr", "st"];

static SENTENCE_BOUNDARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[。！？]+[」』）”’"')\]]*|[.!?…][\s"'”’)\]]|\n"#).expect("padrão de fronteira inválido"));
static LAST_WORD_BEFORE_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\p{L}+)\.$").expect("padrão de abreviatura inválido"));
/// O crate `regex` não suporta backreferences (`</\1>` do JS não tem equivalente direto) — a
/// abertura é reconhecida por esta regex, e [`close_tag_regex`] resolve o fecho correspondente ao
/// NOME capturado, não por backreference.
static REASONING_OPEN_NAMED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<(think|thinking|reasoning)>").expect("padrão de abertura inválido"));
static REASONING_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<(?:think|thinking|reasoning)>").expect("padrão de abertura inválido"));
static CLOSE_THINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</think>").expect("padrão de fecho inválido"));
static CLOSE_THINKING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</thinking>").expect("padrão de fecho inválido"));
static CLOSE_REASONING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</reasoning>").expect("padrão de fecho inválido"));
/// Cerca de bloco de código (` ``` `). Só marca posição — nada aqui troca o conteúdo por deixa
/// nenhuma (isso é trabalho de `tts::preprocessor::sanitize_text`, que roda depois, por chunk); este
/// módulo só decide ONDE cortar, e precisa saber que quebras de linha DENTRO de uma cerca não são
/// fim de frase.
static CODE_FENCE_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```").expect("padrão de cerca inválido"));
static CLOSED_CODE_FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").expect("padrão de cerca fechada inválido"));

fn close_tag_regex(tag_lower: &str) -> &'static Regex {
    match tag_lower {
        "think" => &CLOSE_THINK,
        "thinking" => &CLOSE_THINKING,
        "reasoning" => &CLOSE_REASONING,
        _ => unreachable!("REASONING_OPEN_NAMED só captura essas três variantes"),
    }
}

fn ends_in_abbreviation(text: &str) -> bool {
    let Some(caps) = LAST_WORD_BEFORE_DOT.captures(text.trim_end()) else { return false };
    let word = &caps[1];
    // Inicial isolada ("J. R. R. Tolkien"): uma letra maiúscula sozinha nunca é fim de frase.
    if word.chars().count() == 1 && word.chars().next().is_some_and(char::is_uppercase) {
        return true;
    }
    ABBREVIATIONS.contains(&word.to_lowercase().as_str())
}

/// Remove blocos de raciocínio JÁ FECHADOS (abertura + fecho do MESMO nome, achado sem
/// backreference via [`close_tag_regex`]) de um texto inteiro — usado tanto para a resposta final
/// quanto para o meio do stream, antes de checar se sobrou uma abertura sem fecho.
pub fn strip_reasoning_spans(text: &str) -> String {
    let mut result = String::new();
    let mut pos = 0;
    loop {
        let Some(open_caps) = REASONING_OPEN_NAMED.captures_at(text, pos) else {
            result.push_str(&text[pos..]);
            break;
        };
        let open_match = open_caps.get(0).expect("grupo 0 sempre existe");
        let tag_lower = open_caps[1].to_lowercase();
        let close_re = close_tag_regex(&tag_lower);

        match close_re.find_at(text, open_match.end()) {
            Some(close_match) => {
                result.push_str(&text[pos..open_match.start()]);
                result.push(' ');
                pos = close_match.end();
            }
            // Abertura sem fecho correspondente: preserva o resto intacto (fica para
            // `REASONING_OPEN` decidir o que fica retido em `split_reasoning`).
            None => {
                result.push_str(&text[pos..]);
                break;
            }
        }
    }
    result
}

/// Separa o que pode ser falado do bloco de raciocínio ainda ABERTO, que fica retido até fechar —
/// e, na sequência, de uma cerca de código ainda aberta (mesma ideia, delimitador diferente: uma
/// cerca aberta num chunk e fechada só num delta posterior não pode vazar código pedaço a pedaço).
fn split_reasoning(buffer: &str) -> (String, String) {
    let closed = strip_reasoning_spans(buffer);
    let (before_fence, held_reasoning) = match REASONING_OPEN.find(&closed) {
        Some(open) => (closed[..open.start()].to_string(), closed[open.start()..].to_string()),
        None => (closed, String::new()),
    };
    let (speakable, held_fence) = split_open_code_fence(&before_fence);
    (speakable, format!("{held_fence}{held_reasoning}"))
}

/// Uma cerca sem par (contagem ímpar de ` ``` `) está aberta: tudo da última abertura em diante
/// fica retido, junto — nunca cortado em frases — até o par fechar num delta seguinte.
fn split_open_code_fence(text: &str) -> (String, String) {
    let marks: Vec<usize> = CODE_FENCE_MARK.find_iter(text).map(|m| m.start()).collect();
    if marks.len() % 2 == 1 {
        let open = *marks.last().expect("contagem ímpar implica ao menos uma marca");
        return (text[..open].to_string(), text[open..].to_string());
    }
    (text.to_string(), String::new())
}

/// Faixas de byte de toda cerca de código JÁ FECHADA em `text` — usado só para que
/// [`next_sentence_cut`] nunca aceite uma fronteira (inclusive `\n`) que caia DENTRO de uma: o
/// bloco vira uma unidade só até a próxima chamada de `sanitize_text` decidir o que falar dele.
fn closed_fence_spans(text: &str) -> Vec<(usize, usize)> {
    CLOSED_CODE_FENCE.find_iter(text).map(|m| (m.start(), m.end())).collect()
}

fn inside_a_fence(spans: &[(usize, usize)], byte_idx: usize) -> bool {
    spans.iter().any(|&(start, end)| byte_idx > start && byte_idx < end)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SentenceCut {
    pub ready: Vec<String>,
    pub rest: String,
}

/// Stream parou (o modelo chamou uma ferramenta): fala o que já está no buffer, sem esperar o
/// espaço pós-pontuação que só viria depois da ferramenta. Diferente de `final`: raciocínio ABERTO
/// fica retido.
pub fn flush_speakable(buffer: &str) -> SentenceCut {
    let (speakable, held) = split_reasoning(buffer);
    let tail = speakable.trim();
    SentenceCut { ready: if tail.is_empty() { vec![] } else { vec![tail.to_string()] }, rest: held }
}

fn next_sentence_cut(text: &str, min_chars: usize) -> Option<usize> {
    let fences = closed_fence_spans(text);
    for m in SENTENCE_BOUNDARY.find_iter(text) {
        if inside_a_fence(&fences, m.start()) {
            continue;
        }
        let first_char = text[m.start()..].chars().next().expect("match não pode ser vazio");
        // Pontuação CJK fecha sem espaço depois e leva os fechadores (」』）) junto: o match todo.
        let end = if matches!(first_char, '。' | '！' | '？') { m.end() } else { m.start() + first_char.len_utf8() };
        let candidate = &text[..end];
        if ends_in_abbreviation(candidate) {
            continue;
        }
        if candidate.trim().chars().count() >= min_chars {
            return Some(end);
        }
    }
    None
}

/// Mesma fronteira forçada de sempre, mas nunca DENTRO de uma cerca de código fechada — um bloco
/// sem pontuação por 180+ caracteres (comum: poucas linhas já passam disso) tinha o próprio espaço
/// de indentação como separador válido, cortando o código no meio antes de `next_sentence_cut`
/// sequer entrar em jogo.
fn forced_cut_index(text: &str) -> usize {
    let limit_byte = text.char_indices().nth(MAX_CHUNK_CHARS).map_or(text.len(), |(i, _)| i);
    let window = &text[..limit_byte];
    // Faixas medidas no `text` INTEIRO, não em `window`: uma cerca mais longa que o teto tem o
    // fecho FORA da janela, e ainda assim toda a janela está dentro dela.
    let fences = closed_fence_spans(text);

    for is_separator in [is_comma_semicolon_colon, char::is_whitespace] {
        let found = window.char_indices().rev().find(|&(idx, c)| is_separator(c) && !inside_a_fence(&fences, idx));
        if let Some((byte_idx, c)) = found {
            let char_idx = window[..byte_idx].chars().count();
            if char_idx >= MIN_CHUNK_CHARS {
                return byte_idx + c.len_utf8();
            }
        }
    }
    // Nem vírgula/;/: nem espaço sobraram fora de uma cerca dentro da janela: se o estouro É a
    // cerca (mais longa que o teto), o corte cru cairia no meio dela — empurra até o fecho em vez
    // de truncar por dentro (o chunk sai maior que o teto, intacto, e vira UMA deixa depois).
    match fences.iter().find(|&&(start, end)| limit_byte > start && limit_byte < end) {
        Some(&(_, end)) => end,
        None => limit_byte,
    }
}

fn is_comma_semicolon_colon(c: char) -> bool {
    matches!(c, ',' | ';' | ':' | '、' | '，' | '；' | '：')
}

/// `min_chars` menor na primeira frase do turno: o primeiro áudio sai antes, o resto junta como
/// sempre. `None` usa o piso padrão (`MIN_CHUNK_CHARS`), mesmo default de parâmetro do TS.
pub fn take_complete_sentences(buffer: &str, final_flush: bool, min_chars: Option<usize>) -> SentenceCut {
    let min_chars = min_chars.unwrap_or(MIN_CHUNK_CHARS);
    let (speakable, held) = split_reasoning(buffer);

    if final_flush {
        // Bloco de raciocínio aberto no fim do turno nunca é falado.
        let tail = speakable.trim();
        return SentenceCut { ready: if tail.is_empty() { vec![] } else { vec![tail.to_string()] }, rest: String::new() };
    }

    let mut ready = Vec::new();
    let mut rest = speakable;
    loop {
        if let Some(cut) = next_sentence_cut(&rest, min_chars) {
            ready.push(rest[..cut].trim().to_string());
            rest = rest[cut..].trim_start().to_string();
            continue;
        }

        if rest.chars().count() > MAX_CHUNK_CHARS {
            let forced = forced_cut_index(&rest);
            let candidate = rest[..forced].trim().to_string();
            if !candidate.is_empty() {
                ready.push(candidate);
                rest = rest[forced..].trim_start().to_string();
                continue;
            }
        }
        break;
    }

    SentenceCut { ready, rest: format!("{rest}{held}") }
}

/// Piso da primeira oração: abaixo disto ("Hmm,", "Olha,") o chunk sai picotado e o TTS gasta uma
/// ida inteira num fragmento sem prosódia.
const FIRST_CLAUSE_MIN_CHARS: usize = 20;

/// Só para o PRIMEIRO chunk do turno, quando a frase ainda não fechou: corta na primeira vírgula,
/// ponto e vírgula, dois-pontos ou travessão seguido de espaço com ao menos
/// [`FIRST_CLAUSE_MIN_CHARS`] antes — o primeiro áudio sai uma oração antes do ponto final. Mesmas
/// guardas de [`take_complete_sentences`]: raciocínio aberto e cerca de código ficam retidos.
pub fn take_first_clause(buffer: &str) -> SentenceCut {
    let (speakable, held) = split_reasoning(buffer);
    let fences = closed_fence_spans(&speakable);
    let mut chars = speakable.char_indices().peekable();
    while let Some((idx, c)) = chars.next() {
        let followed_by_space = chars.peek().is_some_and(|&(_, next)| next.is_whitespace());
        let is_clause_end = (followed_by_space && matches!(c, ',' | ';' | ':' | '—' | '–')) || matches!(c, '、' | '，' | '；' | '：');
        if !is_clause_end || inside_a_fence(&fences, idx) {
            continue;
        }
        let end = idx + c.len_utf8();
        if speakable[..end].trim().chars().count() >= FIRST_CLAUSE_MIN_CHARS {
            let rest = speakable[end..].trim_start();
            return SentenceCut { ready: vec![speakable[..end].trim().to_string()], rest: format!("{rest}{held}") };
        }
    }
    SentenceCut { ready: Vec::new(), rest: format!("{speakable}{held}") }
}

// Teto de 300 linhas do AGENTS.md: os casos de teste moram em `stream_sentences_tests.rs`.
#[cfg(test)]
#[path = "stream_sentences_tests.rs"]
mod tests;
