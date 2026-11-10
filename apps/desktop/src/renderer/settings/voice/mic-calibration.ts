import { clamp01 } from '@mika/sdk/utils/clamp'
import { NOISE_MARGIN, scaleVadGates } from '../../src/hooks/speech-transcribe'

/** Level (0..1 on `createMicLevelMeter`'s scale) where the live capture opens the speech gate. */
export function gateLevelForThreshold(threshold: number): number {
  return scaleVadGates(threshold).start
}

/** Inverse of {@link gateLevelForThreshold} over its linear range: a dragged meter line → `voice.voiceActivityThreshold`. */
export function activityThresholdForLevel(level: number): number {
  const full = scaleVadGates(1).start
  return Math.round(clamp01(level / full, 0) * 100) / 100
}

export type MicThresholdSuggestion = {
  noiseFloor: number
  voiceActivityThreshold: number
  vadSpeechThreshold: number
  /** Floor so loud that even the most permissive gate sits under it — the room, not the slider, is the problem. */
  noisy: boolean
}

const SPEECH_PROB_DEFAULT = 0.35
const SPEECH_PROB_MAX = 0.7

function percentile(values: number[], p: number): number {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] ?? 0
}

/**
 * Turns ~3 s of ambient level samples into saved thresholds. The p90 floor ignores a lone spike
 * (door, keyboard) but follows a constant fan; the gate clears it by the runtime's own
 * `NOISE_MARGIN`. The Silero probability (`voice.vadSpeechThreshold`) has no level scale, so it
 * only climbs from the schema default as the room gets louder — fewer false barge-ins over noise.
 */
export function suggestMicThresholds(samples: number[]): MicThresholdSuggestion | null {
  if (samples.length === 0) return null
  const noiseFloor = percentile(samples, 0.9)
  const wantedGate = noiseFloor + NOISE_MARGIN
  const noisy = wantedGate > gateLevelForThreshold(1)
  const speech = Math.min(SPEECH_PROB_MAX, SPEECH_PROB_DEFAULT + Math.max(0, noiseFloor - 0.02) * 2)
  return {
    noiseFloor,
    voiceActivityThreshold: noisy ? 1 : Math.max(0.05, activityThresholdForLevel(wantedGate)),
    vadSpeechThreshold: Math.round(speech * 20) / 20,
    noisy,
  }
}
