/**
 * Checks whether Whisper's text could plausibly have come from this audio.
 * Shared by renderer STT and main-process barge-in recording, which both hit the same stock-phrase
 * hallucination on non-speech audio; measured on a real session log (chars/sec separates cleanly:
 * hallucinations ~0.09-0.52 c/s, real speech ~4.35-11.11 c/s).
 */
const MIN_CHARS_PER_SECOND = 2
/** Other extreme: a degenerate decoder loop repeating the same chunk until it hits the limit. */
const MAX_CHARS_PER_SECOND = 22

/** Audio too short for density to mean anything — two extra chars swing it to 5 c/s. */
const MIN_MEASURABLE_MS = 1_200

/** Below this, text is too short for density to be meaningful (short real speech scores low too). */
const MIN_DENSITY_TEXT_CHARS = 12

/** Stock phrases `whisper-small` returns on non-speech (PT); only count combined with duration/density checks below, since users do say these for real. */
const WHISPER_FILLERS = new Set([
  'e ai',
  'o que e isso',
  'tchau',
  'obrigado',
  'obrigada',
  'ate mais',
  'legendas pela comunidade amara org',
  'valeu',
  'ah',
  'oi',
])
const FILLER_MAX_CHARS_PER_SECOND = 6

/** Duration floor for a stock phrase to count as hallucination; real short speech tops out ~3.5s. */
const FILLER_MIN_HALLUCINATION_MS = 6_000

function normalize(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[^a-z0-9\s]/g, '')
    .trim()
    .replace(/\s+/g, ' ')
}

/** Detects a stuck Whisper decoder repeating the same chunk. */
export function hasDegenerateRepetition(text: string): boolean {
  const words = text.toLowerCase().split(/\s+/).filter(Boolean)
  if (words.length < 8) return false

  for (const size of [1, 2, 3]) {
    if (words.length < size * 4) continue
    const unit = words.slice(0, size).join(' ')
    let repeats = 0
    for (let i = 0; i + size <= words.length; i += size) {
      if (words.slice(i, i + size).join(' ') !== unit) break
      repeats++
    }
    if (repeats >= 4) return true
  }
  return false
}

/**
 * Missing `durationMs` (browser recognition) means "can't judge" — never condemn without it.
 * `minFillerMs` lets fixed-window callers (e.g. barge-in's 2.8s recording) pass their own floor
 * instead of inheriting the renderer's, which that window would never reach.
 */
export function isImplausibleTranscript(
  text: string,
  durationMs?: number,
  options: { minFillerMs?: number } = {},
): boolean {
  if (hasDegenerateRepetition(text)) return true
  if (!durationMs || durationMs < MIN_MEASURABLE_MS) return false

  const density = (text.length / durationMs) * 1000
  if (density > MAX_CHARS_PER_SECOND) return true

  const minFillerMs = options.minFillerMs ?? FILLER_MIN_HALLUCINATION_MS

  // Low density only condemns when it's measuring something: long enough text, or a window too
  // long for such short text (real short speech never exceeds ~3.5s with the silence hangover).
  if (density < MIN_CHARS_PER_SECOND && (text.length >= MIN_DENSITY_TEXT_CHARS || durationMs >= minFillerMs)) {
    return true
  }

  return WHISPER_FILLERS.has(normalize(text)) && density < FILLER_MAX_CHARS_PER_SECOND && durationMs >= minFillerMs
}
