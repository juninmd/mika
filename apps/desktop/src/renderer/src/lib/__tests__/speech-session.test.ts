import { describe, expect, it, vi } from 'vitest'
import { createSpeechSessionController, MIN_RESTART_MS, type SpeechSession } from '../speech-session'

/** Cada ciclo stop→start durante a abertura do microfone não pode vazar sessão de captura. */
describe('speech-session', () => {
  function harness() {
    let clock = 0
    const sessions: Array<{ session: SpeechSession; resolve: () => void; reject: (e: Error) => void }> = []
    const open = vi.fn(
      (_opts: object) =>
        new Promise<SpeechSession>((resolve, reject) => {
          const session = { stop: vi.fn(), setPaused: vi.fn() }
          sessions.push({ session, resolve: () => resolve(session), reject })
        }),
    )
    const onOpenFailed = vi.fn()
    const controller = createSpeechSessionController(open, onOpenFailed, () => clock)
    return {
      controller,
      sessions,
      open,
      onOpenFailed,
      flush: () => new Promise((r) => setTimeout(r, 0)),
      advance: (ms: number) => {
        clock += ms
      },
    }
  }

  it('stop() durante a abertura descarta a sessão quando ela resolve', async () => {
    const { controller, sessions, flush } = harness()
    controller.start({})
    controller.stop()
    sessions[0]?.resolve()
    await flush()

    expect(sessions[0]?.session.stop).toHaveBeenCalledTimes(1)
    expect(controller.isActive()).toBe(false)
  })

  it('stop→start durante a abertura deixa exatamente UMA sessão viva — a mais nova', async () => {
    const { controller, sessions, flush, advance } = harness()
    controller.start({})
    controller.stop()
    advance(MIN_RESTART_MS)
    controller.start({})
    expect(sessions).toHaveLength(2)

    sessions[0]?.resolve()
    sessions[1]?.resolve()
    await flush()

    expect(sessions[0]?.session.stop).toHaveBeenCalledTimes(1)
    expect(sessions[1]?.session.stop).not.toHaveBeenCalled()

    controller.stop()
    expect(sessions[1]?.session.stop).toHaveBeenCalledTimes(1)
  })

  it('a sessão velha que resolve DEPOIS da nova não sobrescreve nem sobrevive', async () => {
    const { controller, sessions, flush, advance } = harness()
    controller.start({})
    controller.stop()
    advance(MIN_RESTART_MS)
    controller.start({})

    sessions[1]?.resolve()
    sessions[0]?.resolve()
    await flush()

    expect(sessions[0]?.session.stop).toHaveBeenCalledTimes(1)
    expect(sessions[1]?.session.stop).not.toHaveBeenCalled()
    controller.setPaused(true)
    expect(sessions[1]?.session.setPaused).toHaveBeenCalledWith(true)
    expect(sessions[0]?.session.setPaused).not.toHaveBeenCalled()
  })

  it('start() repetido enquanto abre não abre um segundo microfone', () => {
    const { controller, open } = harness()
    controller.start({})
    expect(controller.start({})).toBe(false)
    expect(open).toHaveBeenCalledTimes(1)
  })

  it('pausa pedida antes de resolver chega à sessão quando ela instala', async () => {
    const { controller, sessions, flush } = harness()
    controller.start({})
    controller.setPaused(true)
    sessions[0]?.resolve()
    await flush()

    expect(sessions[0]?.session.setPaused).toHaveBeenCalledWith(true)
  })

  it('falha de uma abertura já descartada não derruba a sessão corrente', async () => {
    const { controller, sessions, onOpenFailed, flush, advance } = harness()
    controller.start({})
    controller.stop()
    advance(MIN_RESTART_MS)
    controller.start({})

    sessions[0]?.reject(new Error('NotAllowedError'))
    await flush()

    expect(onOpenFailed).not.toHaveBeenCalled()
    expect(controller.isActive()).toBe(true)

    sessions[1]?.reject(new Error('NotAllowedError'))
    await flush()
    expect(onOpenFailed).toHaveBeenCalledTimes(1)
    expect(controller.isActive()).toBe(false)
  })

  // A StrictMode/effect remount does stop→start inside the cooldown and never asks again: dropping
  // that start left continuous listening dead at boot with the toggle showing "on".
  it('start() dentro do cooldown é adiado até o fim dele, não descartado', () => {
    vi.useFakeTimers()
    try {
      const { controller, open, advance } = harness()
      controller.start({})
      controller.stop()
      advance(MIN_RESTART_MS - 1)
      expect(controller.start({})).toBe(true)
      expect(controller.start({})).toBe(false)
      expect(open).toHaveBeenCalledTimes(1)
      expect(controller.isActive()).toBe(true)

      advance(1)
      vi.advanceTimersByTime(1)
      expect(open).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })

  it('stop() cancela o start adiado pelo cooldown', () => {
    vi.useFakeTimers()
    try {
      const { controller, open } = harness()
      controller.start({})
      controller.stop()
      controller.start({})
      controller.stop()
      vi.advanceTimersByTime(MIN_RESTART_MS * 2)

      expect(open).toHaveBeenCalledTimes(1)
      expect(controller.isActive()).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('passado o cooldown, start() volta a abrir', () => {
    const { controller, open, advance } = harness()
    controller.start({})
    controller.stop()
    advance(MIN_RESTART_MS)
    expect(controller.start({})).toBe(true)

    expect(open).toHaveBeenCalledTimes(2)
    expect(controller.isActive()).toBe(true)
  })

  it('a sessão que resolve fora da geração corrente continua sendo parada', async () => {
    const { controller, sessions, flush, advance } = harness()
    controller.start({})
    controller.stop()
    advance(MIN_RESTART_MS)
    controller.start({})

    sessions[1]?.resolve()
    sessions[0]?.resolve()
    await flush()

    expect(sessions[0]?.session.stop).toHaveBeenCalledTimes(1)
    expect(sessions[1]?.session.stop).not.toHaveBeenCalled()
    expect(controller.isActive()).toBe(true)
  })
})
