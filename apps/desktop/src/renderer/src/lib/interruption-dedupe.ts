/**
 * Barge-in double-reply guard (defects.md "barge-in", item 1).
 *
 * Two independent pipelines can answer the SAME utterance: the Rust VAD barge-in path (detects
 * within ~100ms, captures a fixed 2.5s window, transcribes, and replies via
 * `awareness::on_interruption` / `chat::proactive::send_interruption_reply`) and the renderer's own
 * continuous-dictation STT (`useSpeechRecognition`), which only finishes transcribing once the user
 * goes silent for `SILENCE_HANGOVER_MS`. `send_proactive_unless` already defers to a REAL chat turn
 * that starts WHILE its own LLM call is still in flight (the `stale()`/`chat_turns_started()` check
 * in `chat/proactive.rs`) — the direction that stays uncovered is the reverse: the Rust reply
 * already finished (it is the faster of the two, reacting within ~100ms of the barge-in, well
 * before the renderer's silence-hangover even starts counting), and the renderer's slower turn for
 * the SAME utterance would still fire. This module closes that direction: the renderer defers to a
 * `voice:interruption` event ONLY when it plausibly describes the SAME utterance it is about to
 * send, never merely because one landed recently.
 *
 * **Why time-since-last-event alone is wrong** (code review, blocker): a barge-in can happen, get
 * answered, and then the user says something ELSE entirely up to a few seconds later — a genuinely
 * new utterance, not a duplicate. A pure wall-clock window (the original shape of this guard) would
 * silently drop that new turn too: the mic would go unresponsive to ANY speech for the whole window
 * after EVERY interruption, not just the one utterance that triggered it. The fix correlates by
 * CONTENT: the Rust event already carries the `transcript` it is about to answer with (the same
 * text `chat::proactive::send_interruption_reply` speaks), so a duplicate is only a duplicate when
 * this hook's own transcript plausibly describes that SAME text.
 *
 * The window (8s) is now only a staleness cap — it keeps a match from firing against an
 * interruption so old it can no longer be the same utterance — not the dedupe signal itself. It
 * covers the Rust pipeline's own worst-case latency observed in production — a fixed 2.5s capture
 * plus up to ~8.3s of STT decode for a comparable utterance (smoke-baseline.md, "hearing" section).
 */
export const INTERRUPTION_DEDUPE_WINDOW_MS = 8_000

export type RecentInterruption = { transcript: string; atMs: number }

/** Strips accents/punctuation and collapses whitespace so STT engines that spell the same utterance
 * slightly differently (accents, casing, a trailing "." Whisper adds and the browser API doesn't)
 * still compare equal. */
function normalize(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, '')
    .trim()
    .replace(/\s+/g, ' ')
}

/** Share of `heard`'s words that `reference` already had for this to be the same utterance. */
const SAME_UTTERANCE_WORD_SHARE = 0.6

/**
 * `true` when most of `heard` was already in `reference`. A greeting or one word spelled
 * differently is still the same utterance (smoke 2026-09-27: two replies to "Oi Mika! Que horas
 * são?"); a renderer transcript that carries the question the fixed 2.5 s Rust capture cut off
 * ("para, para" + the question) is not, so it is still sent. An empty side never matches.
 */
export function mostlyAlreadySaid(heard: string, reference: string): boolean {
  const words = normalize(heard).split(' ').filter(Boolean)
  const known = new Set(normalize(reference).split(' ').filter(Boolean))
  if (words.length === 0 || known.size === 0) return false
  return words.filter((w) => known.has(w)).length / words.length >= SAME_UTTERANCE_WORD_SHARE
}

/** Both timestamps are epoch milliseconds (`Date.now()`/Rust's `SystemTime`), never `performance.now()`. */
export function isDuplicateOfRecentInterruption(
  recent: RecentInterruption | null,
  spoken: string,
  nowMs: number,
): boolean {
  if (recent == null) return false
  const elapsed = nowMs - recent.atMs
  if (elapsed < 0 || elapsed > INTERRUPTION_DEDUPE_WINDOW_MS) return false
  return mostlyAlreadySaid(spoken, recent.transcript)
}
