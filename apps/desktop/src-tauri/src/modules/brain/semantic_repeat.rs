//! "The same thing in other words" for proactive lines. The lexical guard (`guards::similarity`)
//! only catches shared words; smoke 2026-09-27 had "Bleach" twice in 15 s and the same terminal
//! joke paraphrased ~15× past it. This compares meaning with the local embedder
//! (`inference::embedder`); without the model it answers "not a repeat" and the lexical guard and
//! the model's own [PASS] still apply.

use std::collections::HashMap;

/// Cosine at or above this is the same line reworded. e5-small's scores are compressed: on 11 pt-BR
/// pairs (2026-09-27) paraphrases scored 0.882-0.918 and unrelated lines 0.836-0.881. 0.9 misses the
/// loosest paraphrase instead of muting a new line — silence is the worse error (see `guards`).
pub const SEMANTIC_REPEAT_MIN: f32 = 0.9;

pub type Embed = fn(&str) -> Option<Vec<f32>>;

pub fn production_embed(text: &str) -> Option<Vec<f32>> {
    crate::modules::inference::embedder::embed(text).ok()
}

/// Vectors of lines already judged, so each recent line is embedded once, not on every check.
#[derive(Default)]
pub struct SemanticCache {
    vectors: HashMap<String, Vec<f32>>,
}

impl SemanticCache {
    /// Recent lines with no cached vector yet — what must be embedded before [`Self::nearest_to`].
    pub fn missing(&self, recent: &[String]) -> Vec<String> {
        recent.iter().filter(|line| !self.vectors.contains_key(*line)).cloned().collect()
    }

    pub fn absorb(&mut self, fresh: Vec<(String, Vec<f32>)>) {
        self.vectors.extend(fresh);
    }

    /// Like `nearest` with the draft already embedded, never embedding: a line without a
    /// cached vector is skipped, as one the model failed to embed always was.
    pub fn nearest_to(&mut self, draft_vec: &[f32], recent: &[String]) -> Option<(String, f32)> {
        self.vectors.retain(|line, _| recent.contains(line));
        let mut best: Option<(String, f32)> = None;
        for line in recent {
            let Some(vector) = self.vectors.get(line) else { continue };
            let score = crate::modules::inference::embedder::similarity(draft_vec, vector);
            if score >= SEMANTIC_REPEAT_MIN && best.as_ref().is_none_or(|(_, s)| score > *s) {
                best = Some((line.clone(), score));
            }
        }
        best
    }
}

/// Embeds each line; one the model cannot embed is left out.
pub fn embed_all(lines: Vec<String>, embed: Embed) -> Vec<(String, Vec<f32>)> {
    lines.into_iter().filter_map(|line| embed(&line).map(|vector| (line, vector))).collect()
}

#[cfg(test)]
impl SemanticCache {
    /// The most similar `recent` line at or above [`SEMANTIC_REPEAT_MIN`], with its score.
    pub fn nearest(&mut self, draft: &str, recent: &[String], embed: Embed) -> Option<(String, f32)> {
        let draft_vec = embed(draft)?;
        let fresh = embed_all(self.missing(recent), embed);
        self.absorb(fresh);
        self.nearest_to(&draft_vec, recent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // Per test thread: tests run in parallel and must not see each other's calls.
    thread_local!(static CALLS: Cell<usize> = const { Cell::new(0) });

    /// Two "meanings": anything about Bleach points one way, everything else the other.
    fn fake_embed(text: &str) -> Option<Vec<f32>> {
        CALLS.with(|c| c.set(c.get() + 1));
        Some(if text.to_lowercase().contains("bleach") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
    }

    fn no_model(_: &str) -> Option<Vec<f32>> {
        None
    }

    #[test]
    fn a_reworded_line_is_caught_and_a_new_subject_is_not() {
        let recent = vec!["Bleach tá ótimo hoje, hein?".to_string()];
        let mut cache = SemanticCache::default();
        let hit = cache.nearest("Voltou pro Bleach de novo?", &recent, fake_embed).expect("same meaning");
        assert_eq!(hit.0, recent[0]);
        assert!(cache.nearest("Bora tomar uma água?", &recent, fake_embed).is_none());
    }

    #[test]
    fn without_the_model_nothing_is_a_repeat() {
        let recent = vec!["Bleach".to_string()];
        assert!(SemanticCache::default().nearest("Bleach", &recent, no_model).is_none());
    }

    #[test]
    fn each_recent_line_is_embedded_once_and_forgotten_when_it_leaves_the_window() {
        let recent = vec!["Bleach!".to_string(), "Olha a hora".to_string()];
        let mut cache = SemanticCache::default();
        let before = CALLS.with(Cell::get);
        cache.nearest("a", &recent, fake_embed);
        cache.nearest("b", &recent, fake_embed);
        assert_eq!(CALLS.with(Cell::get) - before, 4, "2 drafts + 2 recent lines, cached");
        cache.nearest("c", &recent[1..], fake_embed);
        assert_eq!(cache.vectors.len(), 1);
    }
}
