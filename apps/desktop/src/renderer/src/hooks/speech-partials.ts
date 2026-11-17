import type { TranscribeResponse } from './speech-transcribe'

/** Gap between partial STT calls while the user talks: ~1 refresh/s without stacking Whisper runs. */
export const PARTIAL_INTERVAL_MS = 1200

type PartialDeps = {
  transcribe: (pcm: Uint8Array) => Promise<TranscribeResponse>
  /** `''` = speech confirmed, no text yet (the chat shows a typing bubble). */
  publish: (text: string) => void
  /** False while nobody shows partials (chat closed): then no STT is spent on them. */
  enabled: () => boolean
  intervalMs?: number
}

/** Re-transcribes the utterance so far while it is still being spoken, one request at a time. */
export function createPartialTranscriber({
  transcribe,
  publish,
  enabled,
  intervalMs = PARTIAL_INTERVAL_MS,
}: PartialDeps) {
  let utterance = 0
  let announced = false
  let inFlight = false
  let lastAt = 0

  const request = (now: number, pcm: Uint8Array) => {
    const mine = utterance
    inFlight = true
    lastAt = now
    transcribe(pcm)
      .then((result) => {
        const text = result.success ? result.data?.text?.trim() : ''
        if (text && mine === utterance) publish(text)
      })
      .catch(() => {})
      .finally(() => {
        inFlight = false
      })
  }

  return {
    /** Called on VAD ticks once speech is confirmed; `voiceActive` is false during the trailing pause. */
    tick(now: number, voiceActive: boolean, pcm: () => Uint8Array) {
      if (!enabled()) return
      if (!announced) {
        announced = true
        lastAt = now
        publish('')
        return
      }
      if (voiceActive && !inFlight && now - lastAt >= intervalMs) request(now, pcm())
    },
    endUtterance() {
      utterance++
      announced = false
    },
  }
}
