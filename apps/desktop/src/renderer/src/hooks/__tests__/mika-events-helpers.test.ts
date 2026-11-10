import { describe, expect, it } from 'vitest'
import { leadingAvatarTag, sanitizeStreamBatch, stripActionTags } from '../mika-events-helpers'

// The model prefixes replies with emotion/action tags (`<|SMUG|>`, `<|TAG:0.5|>`). The Rust side
// only ever strips them for TTS (speech_text.rs); the avatar reads the opening tag separately
// (`leadingAvatarTag`), so stripping them from the shown text loses nothing.
describe('stripActionTags', () => {
  it('strips a single tag with no numeric arg', () => {
    expect(stripActionTags('<|SMUG|>Oi, tudo bem?')).toBe('Oi, tudo bem?')
  })

  it('strips consecutive tags and the whitespace between them', () => {
    expect(stripActionTags('<|THINKING|> <|SITTING|> deixa eu pensar')).toBe('deixa eu pensar')
  })

  it('strips a tag carrying a numeric intensity argument', () => {
    expect(stripActionTags('<|SMUG:0.5|>com certeza')).toBe('com certeza')
  })

  it('is case-insensitive, matching the Rust (?i) regex', () => {
    expect(stripActionTags('<|smug|>oi')).toBe('oi')
  })

  it('still strips the legacy bracket form', () => {
    expect(stripActionTags('[SMUG] oi')).toBe('oi')
  })

  // Body actions are hyphenated (`throw-tomato`): "[THROW-TOMATO] De novo?" reached the bubble.
  it('strips hyphenated action tags in both forms', () => {
    expect(stripActionTags('[THROW-TOMATO] De novo?')).toBe('De novo?')
    expect(stripActionTags('<|six-seven|>kkk')).toBe('kkk')
  })

  it('collapses leftover double spaces and leading whitespace', () => {
    expect(stripActionTags('<|SMUG|>  <|THINKING|>   oi')).toBe('oi')
  })

  it('never mangles code in a reply: indexing, indentation and blank lines survive', () => {
    const code = 'Assim:\n\nfor (const k in obj) {\n    total += obj[k]\n}'
    expect(stripActionTags(`<|THINKING|> ${code}`)).toBe(code)
  })

  it('leaves ordinary prose untouched', () => {
    expect(stripActionTags('Bora jogar mais uma partida?')).toBe('Bora jogar mais uma partida?')
  })
})

// Streaming flushes the accumulated text, not isolated chunks, so a tag split across two chunks
// (`<|SM` then `UG|>`) has to disappear once both halves are in the same string.
describe('sanitizeStreamBatch — streaming caveat: sanitize the accumulation, not the chunk', () => {
  it('a tag split across two flushes is invisible per-chunk but stripped once accumulated', () => {
    const chunk1 = 'oi <|SM'
    const chunk2 = 'UG|> tudo bem?'

    // Sanitizing each chunk alone leaves the split halves behind — this is the bug this fix avoids.
    expect(sanitizeStreamBatch(chunk1) + sanitizeStreamBatch(chunk2)).toBe('oi <|SMUG|> tudo bem?')

    // Sanitizing the full accumulation (what useMikaEvents.ts now does on every flush) strips it.
    expect(sanitizeStreamBatch(chunk1 + chunk2)).toBe('oi tudo bem?')
  })

  it('is the same function as stripActionTags', () => {
    expect(sanitizeStreamBatch).toBe(stripActionTags)
  })
})

// system-tail.md tells the model to open every reply with a pose/emotion tag, but nothing read it:
// `<|WAVE|>` was stripped from the bubble and the avatar never waved.
describe('leadingAvatarTag', () => {
  it('reads the opening tag as an avatar expression with full intensity', () => {
    expect(leadingAvatarTag('<|WAVE|> Oi!')).toEqual({ expression: 'wave', intensity: 1 })
    expect(leadingAvatarTag('[SITTING] cansei')).toEqual({ expression: 'sitting', intensity: 1 })
  })

  it('normalizes case and underscores to the hyphenated action names', () => {
    expect(leadingAvatarTag('<|thumbs_up|> boa')).toEqual({ expression: 'thumbs-up', intensity: 1 })
    expect(leadingAvatarTag('<|FIST-PUMP|> isso!')).toEqual({ expression: 'fist-pump', intensity: 1 })
  })

  it('carries the numeric intensity, clamped to 0..1', () => {
    expect(leadingAvatarTag('<|HAPPY:0.4|> hm')).toEqual({ expression: 'happy', intensity: 0.4 })
    expect(leadingAvatarTag('<|HAPPY:7|> hm')).toEqual({ expression: 'happy', intensity: 1 })
  })

  it('skips tags the avatar does not know and takes the first one it does', () => {
    expect(leadingAvatarTag('<|JOY|> <|LAUGH|> kkk')).toEqual({ expression: 'laugh', intensity: 1 })
  })

  it('waits while the stream may still complete the tag', () => {
    expect(leadingAvatarTag('')).toBe('pending')
    expect(leadingAvatarTag('<|SM')).toBe('pending')
    expect(leadingAvatarTag('  [CEL')).toBe('pending')
  })

  it('a tag later in the reply is prose, and code indexing is never a tag', () => {
    expect(leadingAvatarTag('Oi <|WAVE|>')).toBeNull()
    expect(leadingAvatarTag('[i] = 0')).toBeNull()
    expect(leadingAvatarTag('<|JOY|> oi')).toBeNull()
  })
})
