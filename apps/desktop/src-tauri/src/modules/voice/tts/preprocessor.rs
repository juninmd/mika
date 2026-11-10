//! Porte fiel de `apps/desktop/src/main/services/voice/tts/preprocessor.ts` — a parte pura
//! (sanitização de texto, normalização de risada, corte em sentenças). `translateText` (DeepLX)
//! FICA DE FORA, e é um corte NOMEADO, não um mock: aquele caminho existe só quando
//! `voice.ttsTranslateAudio=true` (default `false`, `voice.rs` schema) e depende de um servidor
//! HTTP de terceiros rodando em `localhost:1188` — sem esse servidor, o teste seria "rede
//! indisponível" mascarado de sucesso. Quem religar precisa decidir se replica o SSRF-gate de
//! `isDeeplxEndpointAllowed` (`isLoopbackUrl` + `crate::modules::privacy::assert_network_allowed`)
//! ou se aceita a mesma degradação "sem tradução, fala segue" do lado TS.

use regex::Regex;
use std::sync::OnceLock;

/// Regex de padrão fixo, compilada uma vez por ponto de uso. `sanitize_text` roda por CHUNK de fala
/// e compilava sete padrões a cada chamada — a compilação custava mais que a sanitização inteira.
macro_rules! static_regex {
    ($pattern:expr) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($pattern).expect("padrão inválido"))
    }};
}

/// Mesmo alfabeto de `hasSpeakableText` (`\p{L}\p{N}}` unicode) — usado pelo orquestrador para
/// pular chunks que só sobraram pontuação/espaço depois da sanitização.
pub fn has_speakable_text(text: &str) -> bool {
    text.chars().any(|c| c.is_alphanumeric())
}

fn laugh_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)\[(?:risos?|risadas?|laughs?|laughing|giggles?|rindo|gargalhadas?)\]",
            r"(?i)\*(?:risos?|risadas?|laughs?|ri|rindo|gargalha\w*)\*",
            r"(?i)(^|[^a-z])k{3,}([^a-z]|$)",
            r"(?i)(^|[^a-z])(?:rs){2,}([^a-z]|$)",
            r"(?i)(^|[^a-z])(?:ha){2,}h?([^a-z]|$)",
            r"(?i)(^|[^a-z])(?:hue){2,}([^a-z]|$)",
            r"(?i)(^|[^a-z])(?:he){2,}([^a-z]|$)",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("padrão de risada inválido"))
        .collect()
    })
}

/// O que a Mika diz no lugar de ler código em voz alta — uma vez por chamada (ver
/// [`strip_code_fences`]; um segundo bloco na MESMA chamada só some, não repete a deixa).
const CODE_BLOCK_CUE: &str = "Deixei o código aqui no chat.";

/// Bloco cercado (` ```...``` `) inteiro vira a deixa falada; qualquer bloco seguinte na MESMA
/// chamada só some — nunca duas deixas na mesma frase. `sanitize_text` roda por CHUNK (uma frase já
/// cortada, não a resposta inteira — ver `speaker/chunk.rs`), então "uma vez por resposta" é exato
/// no caso comum (um bloco por resposta); duas rodadas de deixa numa resposta com dois blocos
/// separados é o preço aceito de não inventar estado entre chamadas para uma função pura.
fn strip_code_fences(text: &str) -> String {
    static FENCE: OnceLock<Regex> = OnceLock::new();
    let fence = FENCE.get_or_init(|| Regex::new(r"(?s)```.*?```").expect("padrão de bloco de código inválido"));
    let mut said_cue = false;
    fence
        .replace_all(text, |_: &regex::Captures| {
            if said_cue {
                String::new()
            } else {
                said_cue = true;
                CODE_BLOCK_CUE.to_string()
            }
        })
        .to_string()
}

/// `[texto](url)` -> só `texto` falado — roda ANTES de `ignore_brackets`/`ignore_parentheses`
/// (abaixo), que sozinhas apagariam os DOIS lados independentemente e o texto visível do link
/// sumiria junto com a URL em vez de sobrar falável.
fn replace_markdown_links(text: &str) -> String {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(([^)]*)\)").expect("padrão de link markdown inválido"));
    link.replace_all(text, "$1").to_string()
}

/// URL crua (sem ser parte de um link markdown, já resolvido acima) -> "o link", em vez de soletrar
/// o endereço inteiro em voz alta.
fn replace_bare_urls(text: &str) -> String {
    static URL: OnceLock<Regex> = OnceLock::new();
    // Não consome pontuação de fim de frase colada na URL (`.`, `,`, `!`, `?`, `;`, `:`, fechamentos)
    // — sem isso o ponto final "some" junto com o link e as duas sentenças viram uma só depois do split.
    let url = URL.get_or_init(|| {
        Regex::new(r#"(?i)\bhttps?://\S*[^\s.,!?;:)\]"'”’]"#).expect("padrão de URL inválido")
    });
    url.replace_all(text, "o link").to_string()
}

/// `rust::regex` não tem lookbehind — os padrões acima trocam `(?<![a-z])` por `(^|[^a-z])`
/// consumindo o caractere vizinho; os dois vizinhos são capturados e devolvidos na substituição
/// (`${1}`/`${2}`), senão "oi kkkkk tudo" virava "oihahahatudo". O vizinho de trás de uma risada já
/// foi consumido pela anterior ("kkk kkk"), então cada padrão repete até não casar mais.
///
/// Risada vira sempre `hahaha` falado: o estilo `AudioTag` (`[laughs]`) do TS não tem chamador em
/// Rust — nenhum provedor portado o pede.
pub fn normalize_laughter(text: &str) -> String {
    const TOKEN: &str = "LAUGH";
    let mut next = text.to_string();
    for pattern in laugh_patterns() {
        while pattern.is_match(&next) {
            next = pattern.replace_all(&next, format!("${{1}}{TOKEN}${{2}}")).to_string();
        }
    }

    let collapse = static_regex!(&format!(r"(?:{TOKEN}[\s,.!]*)+"));
    next = collapse.replace_all(&next, format!("{TOKEN} ")).to_string();

    next.replace(TOKEN, "hahaha").split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy)]
pub struct TextPreprocessorOptions {
    pub ignore_brackets: bool,
    pub ignore_parentheses: bool,
    pub ignore_asterisks: bool,
    pub ignore_angle_brackets: bool,
    pub remove_emoji: bool,
}

impl Default for TextPreprocessorOptions {
    fn default() -> Self {
        Self {
            ignore_brackets: true,
            ignore_parentheses: true,
            ignore_asterisks: true,
            ignore_angle_brackets: true,
            remove_emoji: true,
        }
    }
}

/// Porte de `sanitizeText`. `limit` corta em CARACTERES (`substring` do TS já é UTF-16 code unit,
/// mas nenhum texto falado real usa astral plane, então `chars().take(limit)` reproduz o efeito
/// observável sem herdar a semântica de UTF-16).
pub fn sanitize_text(text: &str, limit: usize, opts: TextPreprocessorOptions) -> String {
    // Código, link markdown e URL crua resolvidos ANTES de qualquer outra passada: as três trocam
    // uma faixa inteira por texto curto, e rodar depois das passadas de risada/colchete abaixo
    // deixaria a faixa já mutilada por elas (ver o cabeçalho de cada função).
    let next = strip_code_fences(text);
    let next = super::tool_call_text::strip_tool_call_text(&next);
    let next = replace_markdown_links(&next);
    let next = replace_bare_urls(&next);
    let mut next = normalize_laughter(&next);

    next = re_replace(&next, static_regex!(r"(?i)<\|[A-Z_0-9-]+(?::[\d.]+)?\|>"), "");
    next = re_replace(&next, static_regex!(r"\[[A-Z_0-9-]+\]"), "");
    if opts.ignore_brackets {
        next = re_replace(&next, static_regex!(r"\[[^\]]*\]"), " ");
    }
    if opts.ignore_parentheses {
        next = re_replace(&next, static_regex!(r"\([^)]*\)"), " ");
    }
    if opts.ignore_asterisks {
        // Bullets first (`normalize_laughter` already joined the lines): two `* ` markers would
        // otherwise pair up as one `*ação*` span, whose opening star never precedes a space.
        next = re_replace(&next, static_regex!(r"(?:^|\s)\*\s+"), " ");
        next = next.replace("**", " ");
        next = re_replace(&next, static_regex!(r"\*[^*]*\*"), " ");
    }
    if opts.ignore_angle_brackets {
        next = re_replace(&next, static_regex!(r"<[^>]*>"), " ");
    }
    if opts.remove_emoji {
        next = strip_extended_pictographic(&next);
    }

    // `_` sai da lista de símbolos soltos (porte fiel do TS incluía, mas quebrava o item 2 da
    // rodada — "inline code mantém o texto sem os crases": identificadores de código usam `_` o
    // tempo todo, e a lista abaixo é só formatação solta de markdown, não conteúdo de código.
    let cleaned: String = next.chars().map(|c| if "`~#>{}|".contains(c) { ' ' } else { c }).collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.chars().take(limit).collect()
}

fn re_replace(text: &str, pattern: &Regex, replacement: &str) -> String {
    pattern.replace_all(text, replacement).to_string()
}

/// `\p{Extended_Pictographic}` não existe no crate `regex` (não é uma propriedade Unicode
/// suportada); filtra pelos mesmos blocos que cobrem o emoji corrente (emoticons, símbolos
/// diversos, transporte, suplementos e o bloco de apresentação de variação) — mesmo efeito
/// observável para o texto que a Mika fala, sem herdar uma dependência de tabela Unicode nova.
fn strip_extended_pictographic(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            let cp = c as u32;
            !matches!(cp,
                0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x2190..=0x21FF | 0x2B00..=0x2BFF | 0xFE0F | 0x200D
            )
        })
        .collect()
}

// Teto de 300 linhas do AGENTS.md: o corte em sentenças mora em `preprocessor/sentence_split.rs`,
// re-exportado para o caminho público de sempre (`speaker/mod.rs` já importa `split_into_sentences`).
mod sentence_split;
pub use sentence_split::split_into_sentences;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_speakable_text_rejects_pure_punctuation() {
        assert!(!has_speakable_text("... !!"));
        assert!(has_speakable_text("oi"));
    }

    #[test]
    fn normalize_laughter_collapses_repeated_tokens_to_a_single_spoken_form() {
        assert_eq!(normalize_laughter("kkkkk"), "hahaha");
        assert_eq!(normalize_laughter("[risos]"), "hahaha");
    }

    /// A risada no meio da frase consumia o espaço vizinho (o padrão casa o caractere antes/depois
    /// para emular o lookbehind): "oi kkkkk tudo" virava "oihahahatudo", uma palavra só para o TTS.
    #[test]
    fn normalize_laughter_keeps_the_words_around_a_laugh_apart() {
        assert_eq!(normalize_laughter("oi kkkkk tudo"), "oi hahaha tudo");
        assert_eq!(normalize_laughter("que isso rsrs sério"), "que isso hahaha sério");
        assert_eq!(normalize_laughter("kkk kkk"), "hahaha");
    }

    #[test]
    fn sanitize_text_replaces_a_fenced_code_block_with_a_spoken_cue() {
        let out = sanitize_text("Olha só: ```js\nconsole.log(1);\n``` bacana, né?", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "Olha só: Deixei o código aqui no chat. bacana, né?");
    }

    #[test]
    fn sanitize_text_speaks_the_code_cue_only_once_when_two_blocks_land_in_the_same_call() {
        let out = sanitize_text("```js\ncode1();\n``` e ```js\ncode2();\n```", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "Deixei o código aqui no chat. e");
    }

    /// Smoke 2026-09-26: `* ` bullets paired up as an `*ação*` span and the list items were never spoken.
    #[test]
    fn sanitize_text_speaks_asterisk_bullet_items_and_bold_without_the_markers() {
        let out = sanitize_text("Fases:\n* **Admissão:** o pistão desce.\n* Compressão: sobe.", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "Fases: Admissão: o pistão desce. Compressão: sobe.");
    }

    /// "Ela lê as tools": a textual tool call lost only its markers and the JSON body was read aloud.
    #[test]
    fn sanitize_text_never_speaks_a_textual_tool_call() {
        let out = sanitize_text(r#"Já vou ver. <|tool_call|>{"name": "vision_capture", "arguments": {"action": "screenshot"}}"#, 200, TextPreprocessorOptions::default());
        assert_eq!(out, "Já vou ver.");
    }

    #[test]
    fn sanitize_text_keeps_inline_code_text_without_the_backticks() {
        let out = sanitize_text("rodei o `get_system_info` de novo", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "rodei o get_system_info de novo");
    }

    #[test]
    fn sanitize_text_replaces_a_bare_url_with_a_spoken_placeholder() {
        let out = sanitize_text("dá uma olhada em https://example.com/patch?x=1 depois", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "dá uma olhada em o link depois");
    }

    #[test]
    fn sanitize_text_keeps_the_sentence_ending_period_after_a_bare_url() {
        // Regressão: o `\S+` ganancioso comia o ponto final junto com a URL, fundindo
        // duas sentenças em uma só depois do split.
        let out = sanitize_text(
            "veja https://x.com/a. Isso ajuda.",
            200,
            TextPreprocessorOptions::default(),
        );
        assert_eq!(out, "veja o link. Isso ajuda.");
    }

    #[test]
    fn sanitize_text_keeps_only_the_anchor_text_of_a_markdown_link() {
        let out = sanitize_text("olha [o resultado](https://example.com/arquivo.txt) aqui", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "olha o resultado aqui");
    }

    #[test]
    fn sanitize_text_strips_bracketed_and_parenthetical_asides() {
        let out = sanitize_text("oi [aside] (nota) *ação* texto", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "oi texto");
    }

    #[test]
    fn sanitize_text_truncates_to_the_limit() {
        let out = sanitize_text("abcdefghij", 4, TextPreprocessorOptions::default());
        assert_eq!(out, "abcd");
    }

    #[test]
    fn sanitize_text_removes_emoji_when_configured() {
        let out = sanitize_text("oi 😀 tudo bem", 200, TextPreprocessorOptions::default());
        assert_eq!(out, "oi tudo bem");
    }
}
