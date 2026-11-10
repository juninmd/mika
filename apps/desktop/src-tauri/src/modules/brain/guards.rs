//! Guarda que decide se a Mika DEVE falar: repetição. A cadência proativa
//! (horário silencioso, intervalo mínimo, teto por hora) mora em `awareness::gates`.
//!
//! Persona é requisito de produto: guarda de repetição frouxa faz a Mika repetir frase (o
//! comportamento mais "robô" que existe); apertada demais a deixa muda. Por isso os limiares
//! numéricos deste arquivo são cópia EXATA dos originais TypeScript, não estimativa — cada um
//! aponta para a constante que reproduz.
//!
//! ## O que este arquivo porta
//!
//! - [`RepetitionGuard`] — port de
//!   `apps/desktop/src/main/services/brain/repetition/repetition-guard.ts` +
//!   `repetition/similarity.ts`.
//!
//! ## Divergência de paridade (documentada, não acidental)
//!
//! **`strip_diacritics`** usa uma tabela fixa (á/à/â/ã/ä/ç/ñ/…) em vez de NFD Unicode genérico
//! (`text.normalize('NFD').replace(/\p{Diacritic}/gu, '')`). A tabela cobre todo acento usado em
//! pt-BR e nas línguas românicas vizinhas. Para qualquer caractere fora da tabela, o passo seguinte
//! de `normalize_text` (mantém só `[a-z0-9]`) já reduz a espaço — igual ao TS, cujo regex
//! `[^a-z0-9\s]` também descarta sobrevivente não-ASCII.

use std::collections::HashSet;

// ============================================================================================
// Normalização e similaridade de texto — port de `repetition/similarity.ts`.
// ============================================================================================

/// Cobre os acentos que aparecem em pt-BR (e línguas românicas vizinhas). Ver a divergência no
/// cabeçalho do módulo.
fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ý' | 'ÿ' => 'y',
            'Ý' => 'Y',
            other => other,
        })
        .collect()
}

/// = `normalizeText` em `similarity.ts`: acento fora, minúsculo, só `[a-z0-9\s]`, espaço colapsado.
fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // Emotion/pose tags are stage directions, not words: `<|SMUG|>` counted as a topic.
    let spoken = crate::modules::voice::strip_emotion_tags(text);
    for ch in strip_diacritics(&spoken).chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// = `STOPWORDS` em `similarity.ts`. `'é'` é inalcançável (o normalizador já removeu o acento
/// antes de `tokens()` rodar) e mantido só por paridade textual com a lista original.
const STOPWORDS: &[&str] = &[
    "a", "ai", "ao", "aos", "as", "com", "da", "das", "de", "do", "dos", "e", "ea", "em", "eu",
    "isso", "ja", "la", "mais", "mano", "me", "meu", "na", "nao", "nas", "no", "nos", "o", "os",
    "ou", "para", "pra", "pro", "por", "que", "se", "ser", "seu", "so", "sua", "tipo", "ta", "te",
    "tem", "um", "uma", "vc", "vei", "voce", "vou", "é", "ate", "como", "contra", "entre", "mas",
    "sem", "sob", "sobre",
];

fn tokens(normalized: &str) -> Vec<&str> {
    normalized
        .split(' ')
        .filter(|word| word.len() > 1 && !STOPWORDS.contains(word))
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    intersection as f64 / (a.len() + b.len() - intersection) as f64
}

fn word_bigrams(words: &[&str]) -> HashSet<String> {
    if words.len() == 1 {
        return [words[0].to_string()].into_iter().collect();
    }
    let mut grams = HashSet::new();
    for i in 0..words.len().saturating_sub(1) {
        grams.insert(format!("{} {}", words[i], words[i + 1]));
    }
    grams
}

fn char_trigrams(normalized: &str) -> HashSet<String> {
    let compact: Vec<char> = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    let mut grams = HashSet::new();
    if compact.len() < 3 {
        if !compact.is_empty() {
            grams.insert(compact.iter().collect());
        }
        return grams;
    }
    for window in compact.windows(3) {
        grams.insert(window.iter().collect());
    }
    grams
}

/// = `similarity()` em `similarity.ts`. Máximo entre similaridade de bigrama de palavra e trigrama
/// de caractere — o segundo pega paráfrase sem espaço/pontuação igual, o primeiro pega reordenação.
pub fn similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_text(a);
    let norm_b = normalize_text(b);
    if norm_a.is_empty() || norm_b.is_empty() {
        return 0.0;
    }
    if norm_a == norm_b {
        return 1.0;
    }

    let word_score = jaccard(&word_bigrams(&tokens(&norm_a)), &word_bigrams(&tokens(&norm_b)));
    let char_score = jaccard(&char_trigrams(&norm_a), &char_trigrams(&norm_b));
    word_score.max(char_score)
}

fn content_words(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    tokens(&normalized)
        .into_iter()
        .filter(|word| word.len() >= 3)
        .map(str::to_string)
        .collect()
}

/// Preserva a ordem de primeira aparição, como o `Set` do JS (iteração por ordem de inserção) —
/// importa para `saturated_topic` devolver a MESMA palavra que o TS devolveria quando mais de uma
/// palavra do rascunho satura ao mesmo tempo.
fn ordered_unique(words: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for word in words {
        if seen.insert(word.clone()) {
            out.push(word);
        }
    }
    out
}

/// = `saturatedTopic()` com os defaults do TS (`minOccurrences = 3`, `ratio = 0.6`).
pub fn saturated_topic(draft: &str, previous: &[String]) -> Option<String> {
    saturated_topic_with(draft, previous, 3, 0.6)
}

fn saturated_topic_with(
    draft: &str,
    previous: &[String],
    min_occurrences: usize,
    ratio: f64,
) -> Option<String> {
    if previous.len() < min_occurrences {
        return None;
    }
    let draft_words = ordered_unique(content_words(draft));
    if draft_words.is_empty() {
        return None;
    }

    let needed = ((previous.len() as f64 * ratio).ceil() as usize).max(min_occurrences);
    let previous_word_sets: Vec<HashSet<String>> =
        previous.iter().map(|text| content_words(text).into_iter().collect()).collect();

    for word in draft_words {
        let mut hits = 0usize;
        for set in &previous_word_sets {
            if set.contains(&word) {
                hits += 1;
                if hits >= needed {
                    return Some(word);
                }
            }
        }
    }
    None
}

/// = `mostSimilar()`. Limiar INCLUSIVO (`score >= threshold`), e o primeiro candidato empatado
/// vence — mesma regra do TS (`score > best.score`, não `>=`, para desempate).
fn most_similar(text: &str, previous: &[String], threshold: f64) -> Option<(String, f64)> {
    let mut best: Option<(String, f64)> = None;
    for candidate in previous {
        let score = similarity(text, candidate);
        if score < threshold {
            continue;
        }
        let replace = match &best {
            Some((_, best_score)) => score > *best_score,
            None => true,
        };
        if replace {
            best = Some((candidate.clone(), score));
        }
    }
    best
}

/// Words that repeat across lines without being a subject ("agora", "olha"): naming them as covered
/// would forbid filler, not a topic.
const FILLER: &[&str] = &[
    "agora", "aqui", "ainda", "hoje", "olha", "cara", "hein", "gente", "muito", "tudo", "isso", "essa",
    "esse", "esta", "este", "pode", "vamos", "bora", "acho", "sabe", "nada", "mesmo", "outra", "outro",
    "depois", "sempre", "demais", "tambem", "coisa", "novo", "nova", "vez", "quer",
];
const COVERED_TOPICS_MAX: usize = 5;

/// Subjects already covered in `recent`: content words said in at least two different lines, most
/// lines first (ties keep first appearance). Lines alone let the model reword the same subject.
pub fn covered_topics(recent: &[String]) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for line in recent {
        for word in ordered_unique(content_words(line)) {
            if word.len() < 4 || FILLER.contains(&word.as_str()) {
                continue;
            }
            match counts.iter_mut().find(|(seen, _)| *seen == word) {
                Some((_, n)) => *n += 1,
                None => counts.push((word, 1)),
            }
        }
    }
    counts.retain(|(_, n)| *n >= 2);
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    counts.into_iter().take(COVERED_TOPICS_MAX).map(|(word, _)| word).collect()
}

// ============================================================================================
// RepetitionGuard — port de `repetition/repetition-guard.ts`.
// ============================================================================================

/// = `DEFAULT_THRESHOLD` em `repetition-guard.ts`.
pub const REPETITION_DEFAULT_THRESHOLD: f64 = 0.75;
/// 20, not the TS 12: replies share this window and pushed a proactive joke out within minutes, so
/// it came back 20 min later as "new" (2026-09-27).
pub const REPETITION_DEFAULT_WINDOW: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepetitionReason {
    Similar,
    Topic,
    /// Same meaning in other words (`semantic_repeat`), proactive lines only.
    Semantic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepetitionVerdict {
    pub repetitive: bool,
    pub score: f64,
    pub reason: Option<RepetitionReason>,
    pub matched: Option<String>,
    pub topic: Option<String>,
}

impl RepetitionVerdict {
    fn not_repetitive() -> Self {
        Self { repetitive: false, score: 0.0, reason: None, matched: None, topic: None }
    }
}

/// O que decide se um rascunho é repetitivo demais para a Mika falar: parecido com algo recente
/// (`review`'s "similar") ou sobre um assunto já saturado (`"topic"`). Errar para o lado
/// restritivo (emudecer) é pior que repetir — silêncio não deixa rastro em log.
pub struct RepetitionGuard {
    recent: Vec<String>,
    threshold: f64,
    window: usize,
    semantic: super::semantic_repeat::SemanticCache,
    embed: super::semantic_repeat::Embed,
}

/// One history per process, like TS's `repetitionGuard` singleton: user replies and proactive
/// speech share it, so a proactive line can't parrot what Mika just said either way.
pub fn repetition_guard() -> std::sync::MutexGuard<'static, RepetitionGuard> {
    lock(shared_guard())
}

fn shared_guard() -> &'static std::sync::Mutex<RepetitionGuard> {
    static GUARD: std::sync::OnceLock<std::sync::Mutex<RepetitionGuard>> = std::sync::OnceLock::new();
    GUARD.get_or_init(|| std::sync::Mutex::new(RepetitionGuard::new(REPETITION_DEFAULT_THRESHOLD, REPETITION_DEFAULT_WINDOW)))
}

fn lock(guard: &std::sync::Mutex<RepetitionGuard>) -> std::sync::MutexGuard<'_, RepetitionGuard> {
    guard.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// [`approve_proactive_in`] on the process-wide guard.
pub fn approve_proactive(draft: &str) -> bool {
    approve_proactive_in(shared_guard(), draft)
}

/// Rounds of "embed the lines said meanwhile" before deciding with what is cached; a line that
/// arrives after the last round still meets the lexical check, only not the semantic one.
const EMBED_ROUNDS: usize = 3;

/// `RepetitionGuard::approve_proactive` with the embedding done OUTSIDE the lock: `chat::send`
/// takes the same lock to `remember` a reply before emitting it, and a user reply must not wait on
/// a proactive line's embedding. Snapshot, embed unlocked, then lock to re-check and record.
pub fn approve_proactive_in(guard: &std::sync::Mutex<RepetitionGuard>, draft: &str) -> bool {
    let (embed, mut pending) = {
        let locked = lock(guard);
        if locked.lexical_reject(draft) {
            return false;
        }
        (locked.embed, locked.semantic.missing(&locked.recent))
    };
    let trimmed = draft.trim();
    let draft_vec = if trimmed.is_empty() { None } else { embed(trimmed) };
    if draft_vec.is_none() {
        pending.clear();
    }
    for round in 1..=EMBED_ROUNDS {
        let fresh = super::semantic_repeat::embed_all(pending, embed);
        let mut locked = lock(guard);
        locked.semantic.absorb(fresh);
        pending = if draft_vec.is_some() { locked.semantic.missing(&locked.recent) } else { Vec::new() };
        if pending.is_empty() || round == EMBED_ROUNDS {
            return locked.decide(draft, draft_vec.as_deref());
        }
    }
    unreachable!("the last round always decides")
}

impl RepetitionGuard {
    pub fn new(threshold: f64, window: usize) -> Self {
        Self::with_embed(threshold, window, super::semantic_repeat::production_embed)
    }

    pub fn with_embed(threshold: f64, window: usize, embed: super::semantic_repeat::Embed) -> Self {
        Self { recent: Vec::new(), threshold, window, semantic: Default::default(), embed }
    }

    /// Registra `text` no histórico, sem julgar. `remember` e `approve_proactive` compartilham o
    /// mesmo histórico de propósito: o que a Mika já disse numa resposta normal também conta
    /// contra uma fala proativa depois.
    pub fn remember(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        self.recent.push(trimmed.to_string());
        while self.recent.len() > self.window {
            self.recent.remove(0);
        }
    }

    /// What she said lately, oldest first — shown to the model so it can judge repetition by meaning.
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// Julga sem registrar.
    pub fn review(&self, draft: &str) -> RepetitionVerdict {
        let trimmed = draft.trim();

        if let Some((matched, score)) = most_similar(trimmed, &self.recent, self.threshold) {
            return RepetitionVerdict {
                repetitive: true,
                score,
                reason: Some(RepetitionReason::Similar),
                matched: Some(matched),
                topic: None,
            };
        }

        if let Some(topic) = saturated_topic(trimmed, &self.recent) {
            return RepetitionVerdict {
                repetitive: true,
                score: 1.0,
                reason: Some(RepetitionReason::Topic),
                matched: None,
                topic: Some(topic),
            };
        }

        RepetitionVerdict::not_repetitive()
    }

    /// The lexical half of the check, logging a rejection — cheap, so it runs before any embedding.
    fn lexical_reject(&self, draft: &str) -> bool {
        let verdict = self.review(draft);
        if verdict.repetitive {
            log_suppressed(draft, &verdict);
        }
        verdict.repetitive
    }

    /// Re-checks against the CURRENT window (it may have moved while the draft was embedded), then
    /// the meaning check from cached vectors only, and records the draft when it passes.
    fn decide(&mut self, draft: &str, draft_vec: Option<&[f32]>) -> bool {
        if self.lexical_reject(draft) {
            return false;
        }
        if let Some((matched, score)) = draft_vec.and_then(|vector| self.semantic.nearest_to(vector, &self.recent)) {
            let verdict = RepetitionVerdict {
                repetitive: true,
                score: f64::from(score),
                reason: Some(RepetitionReason::Semantic),
                matched: Some(matched),
                topic: None,
            };
            log_suppressed(draft, &verdict);
            return false;
        }
        self.remember(draft);
        true
    }
}

fn log_suppressed(draft: &str, verdict: &RepetitionVerdict) {
    let preview: String = draft.chars().take(120).collect();
    let matched_preview: Option<String> = verdict.matched.as_ref().map(|m| m.chars().take(120).collect());
    log::info!(
        "fala proativa suprimida: repetitiva motivo={:?} score={:.2} draft={:?} parecida_com={:?} assunto_saturado={:?}",
        verdict.reason,
        verdict.score,
        preview,
        matched_preview,
        verdict.topic,
    );
}

impl Default for RepetitionGuard {
    fn default() -> Self {
        Self::new(REPETITION_DEFAULT_THRESHOLD, REPETITION_DEFAULT_WINDOW)
    }
}

#[cfg(test)]
impl RepetitionGuard {
    /// Julga e, se aprovado, registra — [`approve_proactive_in`] sem o `Mutex`, que é o caminho que
    /// a fala proativa usa. Only there the meaning check runs: embedding every reply would cost CPU
    /// on the chat path for nothing.
    pub fn approve_proactive(&mut self, draft: &str) -> bool {
        if self.lexical_reject(draft) {
            return false;
        }
        let trimmed = draft.trim();
        let draft_vec = if trimmed.is_empty() { None } else { (self.embed)(trimmed) };
        if draft_vec.is_some() {
            let fresh = super::semantic_repeat::embed_all(self.semantic.missing(&self.recent), self.embed);
            self.semantic.absorb(fresh);
        }
        self.decide(draft, draft_vec.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- similarity / normalize_text --------------------------------------------------------

    #[test]
    fn normalize_text_strips_accent_case_and_punctuation() {
        assert_eq!(normalize_text("AÇÃO, coração!"), "acao coracao");
        assert_eq!(normalize_text("É AÍ, mano!!"), "e ai mano");
    }

    #[test]
    fn normalize_text_of_empty_input_is_empty() {
        assert_eq!(normalize_text(""), "");
        assert_eq!(normalize_text("   "), "");
    }

    #[test]
    fn normalize_text_reduces_non_latin_unicode_to_spaces_like_the_ts_regex() {
        // Emoji/CJK/cirílico não decompõem em base ASCII nem no TS (\p{Diacritic} não pega) nem
        // aqui — os dois convergem em "vira espaço, depois some no collapse".
        assert_eq!(normalize_text("🎮🎮🎮"), "");
        assert_eq!(normalize_text("こんにちは"), "");
    }

    #[test]
    fn similarity_same_text_is_one_and_accent_insensitive() {
        assert_eq!(similarity("Vamo nessa, MANO!", "vamo nessa mano"), 1.0);
        assert_eq!(similarity("voltou pro CS2?", "voltou pro CS2?"), 1.0);
    }

    #[test]
    fn similarity_empty_text_matches_nothing() {
        assert_eq!(similarity("", "qualquer coisa"), 0.0);
    }

    #[test]
    fn similarity_recognizes_reordering() {
        let score = similarity("Esse boss te amassou de novo?", "De novo esse boss te amassou?");
        assert!(score >= 0.75, "score = {score}");
    }

    #[test]
    fn similarity_separates_different_topics() {
        let score = similarity("E aí, voltou pro CS2?", "Cara, esse merge conflict tá feio demais");
        assert!(score < 0.2, "score = {score}");
    }

    // -- RepetitionGuard ----------------------------------------------------------------------

    #[test]
    fn first_utterance_with_no_history_is_approved_and_then_blocks_the_repeat() {
        let mut guard = RepetitionGuard::default();
        let draft = "esse boss tem um padrão de ataque bem previsível";

        assert!(guard.approve_proactive(draft));
        assert!(!guard.approve_proactive(draft));
    }

    #[test]
    fn different_topic_passes_even_with_full_history() {
        let mut guard = RepetitionGuard::default();
        guard.approve_proactive("esse boss tem um padrão de ataque bem previsível");
        assert!(guard.approve_proactive("quer que eu procure o preço desse teclado?"));
    }

    // Smoke 2026-09-27: the same terminal joke, reworded, got past the word-overlap check ~15 times.
    #[test]
    fn a_proactive_line_with_the_same_meaning_in_other_words_is_blocked() {
        fn by_subject(text: &str) -> Option<Vec<f32>> {
            Some(if text.to_lowercase().contains("terminal") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
        }
        let mut guard = RepetitionGuard::with_embed(REPETITION_DEFAULT_THRESHOLD, REPETITION_DEFAULT_WINDOW, by_subject);
        assert!(guard.approve_proactive("Terminal aberto de novo? Parece hacker de filme."));
        assert!(!guard.approve_proactive("Olha você no terminal outra vez, todo hacker de cinema."));
        assert!(guard.approve_proactive("Que tal uma pausa pra beber água?"));
    }

    #[test]
    fn remember_and_approve_proactive_share_history() {
        let mut guard = RepetitionGuard::default();
        guard.remember("esse boss tem um padrão de ataque bem previsível");
        assert!(!guard.approve_proactive("esse boss tem um padrão de ataque bem previsível"));
    }

    // Nearly every proactive line opens with an emotion tag; read as a word, "smug" saturated the
    // topic and silenced every line after it, a reminder the user asked for included.
    #[test]
    fn an_emotion_tag_is_not_a_topic() {
        let mut guard = RepetitionGuard::default();
        guard.remember("<|SMUG|> Eita, olha esse gato tentando abrir a porta do quarto");
        guard.remember("<|SMUG|> Você zerou o jogo e nem me chamou pra ver o final");
        guard.remember("<|SMUG|> Essa placa de vídeo nova vai fritar tudo aqui");
        assert!(guard.approve_proactive("<|SMUG|> Para de bobeira e vai beber uma água logo"));
    }

    #[test]
    fn empty_draft_is_never_repetitive_and_does_not_dirty_history() {
        let mut guard = RepetitionGuard::default();
        guard.remember("alguma coisa");
        assert!(!guard.review("").repetitive);
        assert!(!guard.review("   ").repetitive);
    }

    #[test]
    fn threshold_is_inclusive_at_the_exact_boundary() {
        let a = "o chefe abre a guarda depois do terceiro golpe";
        let b = "o chefe abre a guarda depois do quarto golpe";
        let score = similarity(a, b);

        let mut at_threshold = RepetitionGuard::new(score, REPETITION_DEFAULT_WINDOW);
        at_threshold.remember(a);
        assert!(at_threshold.review(b).repetitive);

        let mut above_threshold = RepetitionGuard::new(score + 0.01, REPETITION_DEFAULT_WINDOW);
        above_threshold.remember(a);
        assert!(!above_threshold.review(b).repetitive);
    }

    #[test]
    fn the_window_forgets_what_falls_out_of_it() {
        let mut guard = RepetitionGuard::new(0.75, 2);
        let old = "primeira fala da sessão";

        guard.remember(old);
        guard.remember("segunda fala completamente diferente");
        guard.remember("terceira fala sem nada a ver com as outras");

        assert!(!guard.review(old).repetitive);
    }

    #[test]
    fn saturated_topic_blocks_even_when_every_sentence_differs() {
        let mut guard = RepetitionGuard::default();
        guard.remember("o ping desse servidor tá horrível");
        guard.remember("acho que o ping subiu de novo");
        guard.remember("teu ping travou na hora errada");

        let verdict = guard.review("ping de novo, hein");
        assert!(verdict.repetitive);
        assert_eq!(verdict.reason, Some(RepetitionReason::Topic));
        assert_eq!(verdict.topic.as_deref(), Some("ping"));
    }

    #[test]
    fn preposition_is_not_treated_as_a_saturated_topic() {
        let mut guard = RepetitionGuard::default();
        guard.remember("Vi o que você escreveu sobre o deploy de ontem.");
        guard.remember("Aquele vídeo sobre teclado mecânico tava ótimo.");
        guard.remember("Achei curioso o que o cara falou sobre física quântica.");

        assert!(!guard.review("E esse artigo sobre culinária tailandesa?").repetitive);
    }

    #[test]
    fn two_mentions_are_still_conversation_not_a_broken_record() {
        let mut guard = RepetitionGuard::default();
        guard.remember("E aí, voltou pro CS2?");
        guard.remember("Cara, o CS2 de novo? Vicia mesmo.");
        assert!(!guard.review("Bora de CS2 então?").repetitive);
    }

    #[test]
    fn huge_input_does_not_panic_or_hang() {
        let mut guard = RepetitionGuard::default();
        let huge = "palavra ".repeat(20_000);

        let started = std::time::Instant::now();
        guard.remember(&huge);
        assert!(guard.review(&huge).repetitive);
        assert!(started.elapsed().as_secs() < 2, "levou {:?}", started.elapsed());
    }

    #[test]
    fn only_emoji_and_punctuation_matches_nothing() {
        let mut guard = RepetitionGuard::default();
        guard.remember("🎮🎮🎮");
        assert!(!guard.review("!!!").repetitive);
    }

    #[test]
    fn remembering_the_same_line_twice_does_not_consume_two_window_slots() {
        let mut guard = RepetitionGuard::new(0.75, 2);
        let repeated = "mesma fala";

        guard.remember(repeated);
        guard.remember(repeated);
        guard.remember("outra fala qualquer bem diferente");

        assert!(guard.review(repeated).repetitive);
    }

    #[test]
    fn review_alone_never_registers() {
        let guard = RepetitionGuard::default();
        let line = "Alguma coisa aqui";
        assert!(!guard.review(line).repetitive);
        assert!(!guard.review(line).repetitive);
    }

    // A reply's `remember` waited on the proactive embedding: the guard lock was held through it.
    #[test]
    fn the_proactive_check_embeds_without_holding_the_guard_lock() {
        static GUARD: std::sync::OnceLock<std::sync::Mutex<RepetitionGuard>> = std::sync::OnceLock::new();
        static LOCKED_DURING_EMBED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn probe(text: &str) -> Option<Vec<f32>> {
            if GUARD.get().is_some_and(|g| g.try_lock().is_err()) {
                LOCKED_DURING_EMBED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            Some(if text.to_lowercase().contains("terminal") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
        }
        let guard = GUARD.get_or_init(|| std::sync::Mutex::new(RepetitionGuard::with_embed(REPETITION_DEFAULT_THRESHOLD, REPETITION_DEFAULT_WINDOW, probe)));
        assert!(approve_proactive_in(guard, "Terminal aberto de novo? Parece hacker de filme."));
        assert!(!approve_proactive_in(guard, "Olha você no terminal outra vez, todo hacker de cinema."));
        assert!(approve_proactive_in(guard, "Que tal uma pausa pra beber água?"));
        assert!(!LOCKED_DURING_EMBED.load(std::sync::atomic::Ordering::SeqCst), "embedded while holding the guard lock");
    }

    // Embedding outside the lock opens a window; a line said meanwhile must still count.
    #[test]
    fn a_line_remembered_while_the_draft_is_embedded_still_counts() {
        static GUARD: std::sync::OnceLock<std::sync::Mutex<RepetitionGuard>> = std::sync::OnceLock::new();
        fn probe(text: &str) -> Option<Vec<f32>> {
            if text.contains("cinema") {
                let mut guard = GUARD.get().expect("guard").try_lock().expect("embedding must not hold the lock");
                guard.remember("Terminal aberto de novo? Parece hacker de filme.");
            }
            Some(if text.to_lowercase().contains("terminal") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
        }
        let guard = GUARD.get_or_init(|| std::sync::Mutex::new(RepetitionGuard::with_embed(REPETITION_DEFAULT_THRESHOLD, REPETITION_DEFAULT_WINDOW, probe)));
        assert!(!approve_proactive_in(guard, "Olha você no terminal outra vez, todo hacker de cinema."));
        assert_eq!(lock(guard).recent().len(), 1, "the rejected draft is not recorded");
    }

    // Smoke 2026-09-27: reworded lines about the same subject still passed; the novelty prompt
    // needs the subjects, not only the lines.
    #[test]
    fn covered_topics_are_the_subjects_repeated_across_lines_most_frequent_first() {
        let recent = [
            "<|SMUG|> Terminal aberto de novo? Parece hacker de filme.",
            "Olha você no terminal outra vez, todo hacker de cinema.",
            "Ainda no terminal? Bebe uma água agora.",
            "Bora uma partida de CS2 agora?",
        ]
        .map(String::from);
        assert_eq!(covered_topics(&recent), vec!["terminal".to_string(), "hacker".to_string()]);
    }

    #[test]
    fn a_subject_said_once_is_not_covered() {
        assert!(covered_topics(&["Terminal aberto.".to_string(), "Bora beber água?".to_string()]).is_empty());
    }
}
