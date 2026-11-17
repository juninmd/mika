/** What the user is saying right now, before the final transcript: `null` = nobody is speaking. */
export type LiveTranscript = string | null

const listeners = new Set<(text: LiveTranscript) => void>()
let last: LiveTranscript = null

export function publishLiveTranscript(text: LiveTranscript): void {
  if (text === last) return
  last = text
  for (const listener of listeners) listener(text)
}

export function subscribeLiveTranscript(listener: (text: LiveTranscript) => void): () => void {
  listeners.add(listener)
  listener(last)
  return () => {
    listeners.delete(listener)
  }
}

/** Partial STT costs CPU: it only runs while something (the open chat) shows it. */
export function hasLiveTranscriptListeners(): boolean {
  return listeners.size > 0
}
