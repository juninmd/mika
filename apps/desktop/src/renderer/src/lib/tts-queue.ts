// Renderer speech queue overflow policy: full queue drops the NEW chunk, not the old one — losing the middle breaks sentences.

/** Generous cap (many minutes of audio), just to bound a runaway producer. */
export const MAX_TTS_QUEUE = 64

/** Enfileira `item` se couber. Devolve o que aconteceu, para o chamador poder registrar o descarte. */
export function admitToTtsQueue<T>(queue: T[], item: T, max: number = MAX_TTS_QUEUE): 'admitted' | 'dropped' {
  if (queue.length >= max) return 'dropped'
  queue.push(item)
  return 'admitted'
}
