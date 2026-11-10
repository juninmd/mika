export type SpeechSession = {
  stop: () => void
  setPaused: (paused: boolean) => void
}

export type SpeechSessionController<Opts> = {
  // Ignored while a session is already active/opening; inside the restart cooldown it is deferred to
  // the cooldown's end. Return value tells the caller whether to mark "listening".
  start: (opts: Opts) => boolean
  // Stops both the live session and one still opening — the pending one is discarded on resolve.
  stop: () => void
  setPaused: (paused: boolean) => void
  isActive: () => boolean
}

// Minimum window between stop() and the next start(), so effect thrash can't reopen the mic before it closes.
export const MIN_RESTART_MS = 250

// Owns the lifecycle of ONE capture session at a time. getUserMedia is async, so each start() gets a
// generation; a session resolving outside its generation is stopped, not installed — prevents N recorders
// stacking on the same mic when stop() lands mid-open.
export function createSpeechSessionController<Opts>(
  open: (opts: Opts) => Promise<SpeechSession>,
  onOpenFailed: (error: unknown) => void,
  now: () => number = Date.now,
): SpeechSessionController<Opts> {
  let active = false
  let generation = 0
  let pauseWanted = false
  let current: SpeechSession | null = null
  let stoppedAt = Number.NEGATIVE_INFINITY
  let deferred: ReturnType<typeof setTimeout> | undefined

  const install = (mine: number, session: SpeechSession) => {
    if (mine !== generation || !active) {
      session.stop()
      return
    }
    current?.stop()
    current = session
    if (pauseWanted) session.setPaused(true)
  }

  const openNow = (opts: Opts) => {
    deferred = undefined
    const mine = ++generation
    open(opts)
      .then((session) => install(mine, session))
      .catch((error: unknown) => {
        if (mine !== generation) return
        active = false
        onOpenFailed(error)
      })
  }

  const start = (opts: Opts) => {
    if (active) return false
    active = true
    // Effect thrash (stop→start) must not reopen the mic before it closes — but the last start wins.
    const wait = MIN_RESTART_MS - (now() - stoppedAt)
    if (wait > 0) deferred = setTimeout(() => openNow(opts), wait)
    else openNow(opts)
    return true
  }

  const stop = () => {
    clearTimeout(deferred)
    deferred = undefined
    active = false
    generation++
    pauseWanted = false
    stoppedAt = now()
    current?.stop()
    current = null
  }

  const setPaused = (paused: boolean) => {
    pauseWanted = paused
    current?.setPaused(paused)
  }

  return { start, stop, setPaused, isActive: () => active }
}
