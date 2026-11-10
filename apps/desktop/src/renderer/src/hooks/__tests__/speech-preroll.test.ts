import { float32ArrayToInt16 } from '@mika/sdk/utils/audio'
import { describe, expect, it } from 'vitest'
import { createPreRollBuffer, prependPreRoll, preRollDurationMs } from '../speech-preroll'

/** A synthetic "word": a few cycles of a non-zero tone, easy to tell apart from silence (zeros). */
function makeTone(samples: number, amplitude = 0.5): Float32Array {
  const tone = new Float32Array(samples)
  for (let i = 0; i < samples; i++) tone[i] = amplitude * Math.sin(i * 0.3)
  return tone
}

function silence(samples: number): Float32Array {
  return new Float32Array(samples)
}

describe('createPreRollBuffer', () => {
  it('keeps only the most recent maxSamples — old audio falls off the front', () => {
    const buffer = createPreRollBuffer(10)
    buffer.push(new Float32Array([1, 2, 3, 4, 5]))
    buffer.push(new Float32Array([6, 7, 8, 9, 10]))
    buffer.push(new Float32Array([11, 12]))

    expect(Array.from(buffer.snapshot())).toEqual([3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
  })

  it('starts empty and grows as chunks arrive, never exceeding the cap', () => {
    const buffer = createPreRollBuffer(4)
    expect(buffer.snapshot().length).toBe(0)

    buffer.push(new Float32Array([1, 2]))
    expect(buffer.snapshot().length).toBe(2)

    buffer.push(new Float32Array([3, 4, 5]))
    expect(buffer.snapshot().length).toBe(4)
  })

  it('snapshot is a copy — later pushes do not mutate a snapshot already taken', () => {
    const buffer = createPreRollBuffer(10)
    buffer.push(new Float32Array([1, 2, 3]))
    const snap = buffer.snapshot()
    buffer.push(new Float32Array([4, 5, 6]))

    expect(Array.from(snap)).toEqual([1, 2, 3])
  })

  it('reset clears accumulated audio so a later snapshot never replays it', () => {
    // Regression: without a reset, the ring buffer keeps rolling straight through an utterance's
    // own recording. If the next utterance's onset comes within PRE_ROLL_MS of the previous one
    // ending, its snapshot would still hold trailing audio the previous utterance already sent —
    // duplicating already-transcribed speech into the next utterance's PCM.
    const buffer = createPreRollBuffer(10)
    buffer.push(new Float32Array([1, 2, 3, 4, 5]))
    buffer.reset()

    expect(buffer.snapshot().length).toBe(0)

    buffer.push(new Float32Array([6, 7]))
    expect(Array.from(buffer.snapshot())).toEqual([6, 7])
  })
})

describe('prependPreRoll', () => {
  it('a word that starts before the gate crosses is fully present in the emitted utterance', () => {
    // The onset-clipping bug: MediaRecorder only starts once the amplitude gate opens, so "Oi" (the
    // word buffered here as a pre-roll tone) would otherwise never reach the backend — only "Mika"
    // (the silence stand-in for the post-gate recording) would.
    const onsetWord = makeTone(160) // 10ms @ 16kHz
    const postGateAudio = silence(320) // 20ms @ 16kHz — what the recorder actually captured

    const pcm = new Uint8Array(Int16Array.from(postGateAudio).buffer)
    const combined = prependPreRoll(pcm, onsetWord)

    const expectedPreRollBytes = new Uint8Array(float32ArrayToInt16(onsetWord).buffer)
    expect(combined.length).toBe(expectedPreRollBytes.length + pcm.length)
    expect(combined.slice(0, expectedPreRollBytes.length)).toEqual(expectedPreRollBytes)
    expect(combined.slice(expectedPreRollBytes.length)).toEqual(pcm)
  })

  it('passes the recorded PCM through untouched when there is no pre-roll to prepend', () => {
    const pcm = new Uint8Array([1, 2, 3, 4])
    expect(prependPreRoll(pcm, undefined)).toBe(pcm)
    expect(prependPreRoll(pcm, new Float32Array(0))).toBe(pcm)
  })
})

describe('preRollDurationMs', () => {
  it('converts sample count at 16kHz to milliseconds', () => {
    expect(preRollDurationMs(new Float32Array(8_000))).toBe(500)
    expect(preRollDurationMs(undefined)).toBe(0)
    expect(preRollDurationMs(new Float32Array(0))).toBe(0)
  })
})
