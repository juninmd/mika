import { float32ArrayToInt16 } from '@mika/sdk/utils/audio'

// Onset clipping: MediaRecorder only starts once the amplitude gate opens
// (useSpeechRecognition.media.ts's beginUtterance), so the syllable that TRIPPED the gate — "Oi" in
// "Oi Mika" — is never in what gets recorded. This rolling buffer is fed continuously (before the
// gate opens) by the same 16kHz mono tap the barge-in path already uses (vad-processor.worklet.js),
// snapshotted the instant the gate opens, and prepended to the utterance's PCM16 before it goes out.

const PCM_SAMPLE_RATE = 16_000

/** 500-700ms window: long enough to hold a clipped onset syllable, short enough to stay silence/noise. */
export const PRE_ROLL_MS = 600
export const PRE_ROLL_SAMPLES = Math.round((PRE_ROLL_MS / 1000) * PCM_SAMPLE_RATE)

export type PreRollBuffer = {
  push: (chunk: Float32Array) => void
  snapshot: () => Float32Array
  /** Drops accumulated audio — called when an utterance finishes, so a next onset that opens the
   * gate again before the ring naturally refills never replays trailing audio the finished
   * utterance already sent to the backend. */
  reset: () => void
}

/** FIFO ring over the last `maxSamples` — old audio falls off the front as new audio is pushed. */
export function createPreRollBuffer(maxSamples: number = PRE_ROLL_SAMPLES): PreRollBuffer {
  let buffer = new Float32Array(0)
  return {
    push(chunk) {
      const merged = new Float32Array(buffer.length + chunk.length)
      merged.set(buffer)
      merged.set(chunk, buffer.length)
      buffer = merged.length > maxSamples ? merged.slice(merged.length - maxSamples) : merged
    },
    snapshot: () => buffer.slice(),
    reset: () => {
      buffer = new Float32Array(0)
    },
  }
}

/** Pre-roll first, PCM16 bytes — the shape sent over `mika:voice-transcribe`. */
export function prependPreRoll(pcm: Uint8Array, preRoll?: Float32Array): Uint8Array {
  if (!preRoll || preRoll.length === 0) return pcm
  const preRollBytes = new Uint8Array(float32ArrayToInt16(preRoll).buffer)
  const combined = new Uint8Array(preRollBytes.length + pcm.length)
  combined.set(preRollBytes, 0)
  combined.set(pcm, preRollBytes.length)
  return combined
}

export function preRollDurationMs(preRoll?: Float32Array): number {
  return preRoll && preRoll.length > 0 ? Math.round((preRoll.length / PCM_SAMPLE_RATE) * 1000) : 0
}
