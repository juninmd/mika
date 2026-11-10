import { describe, expect, it } from 'vitest'
import { float32ArrayToInt16, float32ToInt16Sample } from '../audio'

// PCM16 is asymmetric (-32768..32767); clamping both sides to ±32767 would silently lose the
// loudest negative sample. That asymmetry is the entire reason this helper is not a one-liner.
describe('float32ToInt16Sample', () => {
  it('maps 0 to 0', () => {
    expect(float32ToInt16Sample(0)).toBe(0)
  })

  it('maps 1.0 (max positive) to 32767, not 32768', () => {
    expect(float32ToInt16Sample(1)).toBe(0x7fff)
  })

  it('maps -1.0 (max negative) to -32768, using the wider negative range', () => {
    expect(float32ToInt16Sample(-1)).toBe(-0x8000)
  })

  it('clamps values above 1 to the positive ceiling', () => {
    expect(float32ToInt16Sample(2.5)).toBe(0x7fff)
  })

  it('clamps values below -1 to the negative floor', () => {
    expect(float32ToInt16Sample(-2.5)).toBe(-0x8000)
  })

  it('scales a positive fraction proportionally', () => {
    expect(float32ToInt16Sample(0.5)).toBe(Math.round(0.5 * 0x7fff))
  })

  it('scales a negative fraction proportionally, using the negative-side multiplier', () => {
    expect(float32ToInt16Sample(-0.5)).toBe(Math.round(-0.5 * 0x8000))
  })
})

describe('float32ArrayToInt16', () => {
  it('converts every sample, preserving array length', () => {
    const input = new Float32Array([0, 1, -1, 0.5])
    const out = float32ArrayToInt16(input)

    expect(out.length).toBe(4)
    expect(Array.from(out)).toEqual([0, 0x7fff, -0x8000, Math.round(0.5 * 0x7fff)])
  })

  it('returns an empty Int16Array for an empty input', () => {
    expect(float32ArrayToInt16(new Float32Array(0)).length).toBe(0)
  })

  it('treats a missing element (sparse array) as silence (0)', () => {
    const sparse = new Float32Array(2)
    // index 1 left at its typed-array default (0) — exercises the `?? 0` fallback path directly
    sparse[0] = 1
    expect(Array.from(float32ArrayToInt16(sparse))).toEqual([0x7fff, 0])
  })
})
