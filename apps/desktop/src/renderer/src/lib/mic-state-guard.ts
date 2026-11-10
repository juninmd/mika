// Anti-ping-pong guard for mic state: the renderer's own echo must not reapply micEnabled, but an
// external change (LLM calling voice:listen_start) must — the distinction is by value. A sliding
// window is the second belt: a burst above `limit` sends trips a circuit breaker.

type MicStateGuard = {
  // false once the send limit in the window is exceeded; only records the send when true.
  shouldSend: (value: boolean, at: number) => boolean
  // false when value is the echo of the last value the renderer sent; true for an external change.
  shouldApplyEcho: (value: boolean, at: number) => boolean
  // ms until a blocked send fits in the window again (0 = now); the caller resends then, never drops.
  retryAfterMs: (at: number) => number
  reset: () => void
}

export function createMicStateGuard({ limit, windowMs }: { limit: number; windowMs: number }): MicStateGuard {
  // Last value the renderer sent to main; null = nothing sent yet, so nothing is an echo.
  let lastSent: boolean | null = null
  let sentAt: number[] = []

  const shouldSend = (value: boolean, at: number): boolean => {
    sentAt = sentAt.filter((t) => at - t < windowMs)
    if (sentAt.length >= limit) return false
    sentAt.push(at)
    lastSent = value
    return true
  }

  // `_at` exists for contract symmetry with shouldSend; the echo decision is by value alone.
  const shouldApplyEcho = (value: boolean, _at: number): boolean => value !== lastSent

  const retryAfterMs = (at: number): number => {
    const live = sentAt.filter((t) => at - t < windowMs)
    return live.length < limit ? 0 : live[live.length - limit] + windowMs - at
  }

  const reset = (): void => {
    lastSent = null
    sentAt = []
  }

  return { shouldSend, shouldApplyEcho, retryAfterMs, reset }
}
