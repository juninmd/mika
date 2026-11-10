import { describe, expect, it } from 'vitest'
import { scaleVadGates } from '../../../src/hooks/speech-transcribe'
import { activityThresholdForLevel, gateLevelForThreshold, suggestMicThresholds } from '../mic-calibration'

describe('mic calibration', () => {
  // The line drawn on the meter must sit exactly where the live capture opens the gate, or dragging
  // it would promise a sensitivity the STT never uses.
  it('places the threshold line on the same level the live VAD opens at', () => {
    for (const t of [0.1, 0.5, 0.8, 1]) {
      expect(gateLevelForThreshold(t)).toBeCloseTo(scaleVadGates(t).start, 5)
    }
  })

  it('round-trips a dragged level back to the saved key', () => {
    expect(activityThresholdForLevel(gateLevelForThreshold(0.6))).toBeCloseTo(0.6, 2)
    expect(activityThresholdForLevel(5)).toBe(1)
    expect(activityThresholdForLevel(-1)).toBe(0)
  })

  it('returns nothing when no sample was captured (mic denied or silent stream)', () => {
    expect(suggestMicThresholds([])).toBeNull()
  })

  it('keeps a quiet room close to the schema defaults', () => {
    const quiet = Array.from({ length: 30 }, (_, i) => 0.01 + (i % 3) * 0.002)
    const s = suggestMicThresholds(quiet)
    expect(s).not.toBeNull()
    expect(s?.noisy).toBe(false)
    expect(s?.vadSpeechThreshold).toBe(0.35)
    // Gate must clear the noise peaks by the runtime's own margin.
    expect(gateLevelForThreshold(s?.voiceActivityThreshold ?? 0)).toBeGreaterThan(0.014)
  })

  it('raises both thresholds in a noisy room and ignores a single spike', () => {
    const fan = Array.from({ length: 30 }, () => 0.08)
    fan[5] = 0.9 // a door slam during sampling must not push the gate to the ceiling
    const noisy = suggestMicThresholds(fan)
    const quiet = suggestMicThresholds(Array.from({ length: 30 }, () => 0.01))
    expect(noisy?.voiceActivityThreshold).toBeGreaterThan(quiet?.voiceActivityThreshold ?? 1)
    expect(noisy?.vadSpeechThreshold).toBeGreaterThan(0.35)
    expect(noisy?.noiseFloor).toBeCloseTo(0.08, 2)
  })

  it('flags a floor too loud for any gate instead of silently saturating', () => {
    const s = suggestMicThresholds(Array.from({ length: 30 }, () => 0.3))
    expect(s?.noisy).toBe(true)
    expect(s?.voiceActivityThreshold).toBe(1)
    expect(s?.vadSpeechThreshold).toBeLessThanOrEqual(0.7)
  })
})
