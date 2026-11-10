import { describe, expect, it } from 'vitest'
import { isLikelyBase64, normalizeBase64 } from '../audio-playback-codec'

describe('isLikelyBase64 — guards the TTS payload before decoding', () => {
  it('accepts a well-formed base64 string', () => {
    expect(isLikelyBase64('AAAAAAAA')).toBe(true)
  })

  it('rejects empty input — nothing to decode', () => {
    expect(isLikelyBase64('')).toBe(false)
  })

  it('rejects a payload shorter than 8 chars — too short to be real audio', () => {
    expect(isLikelyBase64('AAAA')).toBe(false)
  })

  it('rejects a length not a multiple of 4 — malformed base64', () => {
    expect(isLikelyBase64('AAAAAAA')).toBe(false)
  })

  it('rejects characters outside the base64 alphabet — e.g. a JSON error body mistaken for audio', () => {
    expect(isLikelyBase64('{"error":"bad"}')).toBe(false)
  })

  it('tolerates embedded whitespace before validating', () => {
    expect(isLikelyBase64('AAAA AAAA')).toBe(true)
  })
})

describe('normalizeBase64 — strips data-URL prefix and whitespace', () => {
  it('passes through a plain base64 string unchanged', () => {
    expect(normalizeBase64('AAAABBBB')).toBe('AAAABBBB')
  })

  it('strips a data: URL prefix up to the comma', () => {
    expect(normalizeBase64('data:audio/mpeg;base64,AAAABBBB')).toBe('AAAABBBB')
  })

  it('removes internal whitespace/newlines from a wrapped payload', () => {
    expect(normalizeBase64('AAAA\nBBBB CCCC')).toBe('AAAABBBBCCCC')
  })

  it('handles a payload with a comma but empty data segment', () => {
    expect(normalizeBase64('data:audio/mpeg;base64,')).toBe('')
  })
})
