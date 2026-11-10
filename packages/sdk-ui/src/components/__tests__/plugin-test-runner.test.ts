import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPluginTestRunner, type PluginTestResponse, type PluginTestRunnerOptions } from '../plugin-test-runner'
import type { TestStatus } from '../TestResultNote'

function setup(overrides: Partial<PluginTestRunnerOptions> = {}) {
  const states: Array<[TestStatus, string | null]> = []
  const options: PluginTestRunnerOptions = {
    testFn: vi.fn(async () => ({ ok: true })),
    resetDelayMs: 1000,
    runningMessage: 'Testing',
    ...overrides,
  }
  const runner = createPluginTestRunner(
    () => options,
    (status, message) => states.push([status, message]),
  )
  return { runner, states, options, last: () => states.at(-1) }
}

function deferred() {
  let resolve!: (value: PluginTestResponse) => void
  const promise = new Promise<PluginTestResponse>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('createPluginTestRunner', () => {
  it('shows the running message, then the provider message on success', async () => {
    const { runner, states } = setup({ testFn: async () => ({ ok: true, message: 'Ollama 0.9 up' }) })
    await runner.run()
    expect(states).toEqual([
      ['idle', null],
      ['testing', 'Testing'],
      ['ok', 'Ollama 0.9 up'],
    ])
  })

  it('falls back to a default message for a bare ok or failure', async () => {
    const ok = setup({ testFn: async () => ({ ok: true }) })
    await ok.runner.run()
    expect(ok.last()).toEqual(['ok', 'Conexão estabelecida com sucesso!'])
    const fail = setup({ testFn: async () => ({ ok: false }) })
    await fail.runner.run()
    expect(fail.last()).toEqual(['error', 'Falha no teste.'])
  })

  it('turns a thrown error into an error state carrying its message', async () => {
    const { runner, last } = setup({
      testFn: async () => {
        throw new Error('ECONNREFUSED')
      },
    })
    await runner.run()
    expect(last()).toEqual(['error', 'Erro: ECONNREFUSED'])
  })

  it('plays returned audio only when a player is wired', async () => {
    const onAudio = vi.fn()
    const { runner } = setup({ testFn: async () => ({ ok: true, audio: 'UklGRg==' }), onAudio })
    await runner.run()
    expect(onAudio).toHaveBeenCalledWith('UklGRg==')
    await expect(setup({ testFn: async () => ({ ok: true, audio: 'x' }) }).runner.run()).resolves.toBeUndefined()
  })

  it('returns to idle after the reset delay, and never when the delay is zero', async () => {
    const timed = setup()
    await timed.runner.run()
    vi.advanceTimersByTime(1000)
    expect(timed.last()).toEqual(['idle', null])

    const sticky = setup({ resetDelayMs: 0 })
    await sticky.runner.run()
    vi.advanceTimersByTime(60_000)
    expect(sticky.last()?.[0]).toBe('ok')
  })

  it('drops the outcome of a run superseded by a newer one', async () => {
    const slow = deferred()
    const calls = [slow.promise, Promise.resolve({ ok: true, message: 'second' })]
    const { runner, states, last } = setup({ testFn: () => calls.shift() as Promise<PluginTestResponse> })
    const first = runner.run()
    await runner.run()
    slow.resolve({ ok: false, message: 'first' })
    await first
    expect(last()).toEqual(['ok', 'second'])
    expect(states.some(([, message]) => message === 'first')).toBe(false)
  })

  it('reads the latest options on every run', async () => {
    const { runner, options, last } = setup()
    options.testFn = async () => ({ ok: false, message: 'new key rejected' })
    await runner.run()
    expect(last()).toEqual(['error', 'new key rejected'])
  })

  it('dispose cancels a pending reset so an unmounted panel is not updated', async () => {
    const { runner, states } = setup()
    await runner.run()
    const count = states.length
    runner.dispose()
    vi.advanceTimersByTime(5000)
    expect(states).toHaveLength(count)
  })
})
