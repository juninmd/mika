import { describe, expect, it } from 'vitest'
import {
  INTERRUPTION_DEDUPE_WINDOW_MS,
  isDuplicateOfRecentInterruption,
  mostlyAlreadySaid,
} from '../interruption-dedupe'

describe('isDuplicateOfRecentInterruption', () => {
  it('is never a duplicate when no interruption was ever seen', () => {
    expect(isDuplicateOfRecentInterruption(null, 'para de fazer isso', 1_000)).toBe(false)
  })

  it('is a duplicate right after the interruption event lands, for the SAME transcript', () => {
    const recent = { transcript: 'para de fazer isso', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'para de fazer isso', 1_050)).toBe(true)
  })

  it('is still a duplicate at the exact edge of the window', () => {
    const recent = { transcript: 'para de fazer isso', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'para de fazer isso', 1_000 + INTERRUPTION_DEDUPE_WINDOW_MS)).toBe(
      true,
    )
  })

  it('is no longer a duplicate one millisecond past the window (staleness cap)', () => {
    const recent = { transcript: 'para de fazer isso', atMs: 1_000 }
    expect(
      isDuplicateOfRecentInterruption(recent, 'para de fazer isso', 1_000 + INTERRUPTION_DEDUPE_WINDOW_MS + 1),
    ).toBe(false)
  })

  it('is never a duplicate of an interruption that has not happened yet (clock skew guard)', () => {
    // A transcript timestamped BEFORE the interruption we are comparing against is not the same
    // utterance racing ahead — treating it as a duplicate would silently drop an unrelated turn.
    const recent = { transcript: 'para de fazer isso', atMs: 2_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'para de fazer isso', 1_000)).toBe(false)
  })

  // --- blocker (code review): content must correlate, a wall-clock window alone is not enough ---

  it('is NOT a duplicate when a genuinely new, unrelated utterance lands inside the window', () => {
    // This is the regression: before the fix, ANY speech within 8s of ANY interruption was
    // discarded, so a real new thing the user says right after a barge-in never reached the LLM.
    const recent = { transcript: 'para de fazer isso', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'qual é a capital da frança', 1_500)).toBe(false)
  })

  it('is a duplicate when the renderer decoded less of the same utterance (partial decode)', () => {
    const recent = { transcript: 'espera, eu ainda tô falando', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'espera, eu ainda', 1_200)).toBe(true)
  })
  it('is NOT a duplicate when the renderer heard more than the Rust capture window', () => {
    // Smoke 2026-09-26: the Rust reply only heard the fixed 2.5 s after the barge-in ("para, para"),
    // so dropping the renderer's full turn lost the actual question.
    const recent = { transcript: 'Para, para!', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'Para, para! Me fala só qual é a cor do céu.', 1_200)).toBe(false)
  })

  it('is a duplicate when the renderer only adds a greeting the Rust capture also answered', () => {
    // Smoke 2026-09-27: "Oi Mika! Que horas são agora?" got two replies, one per pipeline.
    const recent = { transcript: 'que horas são agora', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'Oi Mika! Que horas são agora?', 1_200)).toBe(true)
  })

  it('is a duplicate when the two decoders spell one word differently', () => {
    const recent = { transcript: 'oi mika que horas sao agora', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'Oi Mika, que hora são agora?', 1_200)).toBe(true)
  })

  it('is never a duplicate when the interruption transcript is empty (silence-endpoint payload)', () => {
    const recent = { transcript: '', atMs: 1_000 }
    expect(isDuplicateOfRecentInterruption(recent, 'oi mika', 1_050)).toBe(false)
  })
})

describe('mostlyAlreadySaid', () => {
  it('matches regardless of accents, case and punctuation', () => {
    expect(mostlyAlreadySaid('Pára, espera!', 'para espera')).toBe(true)
  })

  it('does not match unrelated content', () => {
    expect(mostlyAlreadySaid('para de fazer isso', 'qual é a capital da frança')).toBe(false)
  })

  it('does not match when either side is empty or only punctuation', () => {
    expect(mostlyAlreadySaid('', 'oi')).toBe(false)
    expect(mostlyAlreadySaid('oi', '...')).toBe(false)
  })
})
