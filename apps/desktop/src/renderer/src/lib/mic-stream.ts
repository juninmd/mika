export function stopStream(stream: MediaStream | null | undefined): void {
  for (const track of stream?.getTracks() ?? []) track.stop()
}

/** Monotonic token: starting a new session (or invalidating) retires every earlier one still in flight. */
export function createGenerationGuard() {
  let generation = 0
  return {
    begin: () => ++generation,
    invalidate: () => {
      generation++
    },
    isCurrent: (token: number) => token === generation,
  }
}

/**
 * Opens the mic for a session that may be cancelled while `getUserMedia` is pending. A stream that
 * resolves for a stale session is stopped on the spot (nobody would ever close it) and `null` returned.
 */
export async function openMicIfCurrent(
  open: () => Promise<MediaStream>,
  isCurrent: () => boolean,
): Promise<MediaStream | null> {
  const stream = await open()
  if (isCurrent()) return stream
  stopStream(stream)
  return null
}
