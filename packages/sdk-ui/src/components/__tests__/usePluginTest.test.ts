import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from './fake-react'

vi.mock('react', () => import('./fake-react'))

const { usePluginTest } = await import('../usePluginTest')
type UsePluginTestOptions = Parameters<typeof usePluginTest>[0]

function setup(overrides: Partial<UsePluginTestOptions> = {}) {
  const options: UsePluginTestOptions = {
    testFn: vi.fn(async () => ({ ok: true })),
    resetDelayMs: 50,
    ...overrides,
  }
  const hook = renderHook((props: UsePluginTestOptions) => usePluginTest(props), options)
  return { hook, options }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('usePluginTest', () => {
  it('starts idle and not testing', () => {
    const { hook } = setup()
    expect(hook.current.status).toBe('idle')
    expect(hook.current.message).toBeNull()
    expect(hook.current.isTesting).toBe(false)
  })

  it('runs through testing then the resolved status, driving isTesting off status', async () => {
    const { hook } = setup({ testFn: async () => ({ ok: true, message: 'up' }) })
    const done = hook.current.runTest()
    hook.flush()
    expect(hook.current.status).toBe('testing')
    expect(hook.current.isTesting).toBe(true)
    await vi.advanceTimersByTimeAsync(0)
    await done
    hook.flush()
    expect(hook.current.status).toBe('ok')
    expect(hook.current.message).toBe('up')
    expect(hook.current.isTesting).toBe(false)
  })

  it('turns a rejected testFn into an error message', async () => {
    const { hook } = setup({ testFn: async () => Promise.reject(new Error('offline')) })
    const done = hook.current.runTest()
    await vi.advanceTimersByTimeAsync(0)
    await done
    hook.flush()
    expect(hook.current.status).toBe('error')
    expect(hook.current.message).toBe('Erro: offline')
  })

  it('reset() clears status and message directly, without calling testFn', () => {
    const testFn = vi.fn(async () => ({ ok: true }))
    const { hook } = setup({ testFn })
    hook.act(() => hook.current.reset())
    expect(hook.current).toMatchObject({ status: 'idle', message: null })
    expect(testFn).not.toHaveBeenCalled()
  })

  it('picks up a testFn replaced on rerender, not the one captured at mount', async () => {
    const first = vi.fn(async () => ({ ok: true, message: 'first' }))
    const { hook, options } = setup({ testFn: first })
    const second = vi.fn(async () => ({ ok: true, message: 'second' }))
    hook.rerender({ ...options, testFn: second })
    const done = hook.current.runTest()
    await vi.advanceTimersByTimeAsync(0)
    await done
    hook.flush()
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledOnce()
    expect(hook.current.message).toBe('second')
  })

  it('plays the returned audio only when an onAudio callback was given', async () => {
    const onAudio = vi.fn()
    const { hook } = setup({ testFn: async () => ({ ok: true, audio: 'UklGRg==' }), onAudio })
    const done = hook.current.runTest()
    await vi.advanceTimersByTimeAsync(0)
    await done
    expect(onAudio).toHaveBeenCalledWith('UklGRg==')
  })

  it('auto-resets to idle after resetDelayMs, using the running message meanwhile', async () => {
    const { hook } = setup({ resetDelayMs: 50, runningMessage: 'Aguarde...' })
    const done = hook.current.runTest()
    hook.flush()
    expect(hook.current.message).toBe('Aguarde...')
    await vi.advanceTimersByTimeAsync(0)
    await done
    hook.flush()
    expect(hook.current.status).toBe('ok')
    await vi.advanceTimersByTimeAsync(50)
    hook.flush()
    expect(hook.current.status).toBe('idle')
    expect(hook.current.message).toBeNull()
  })

  it('disposes the pending auto-reset on unmount, so the status never reverts after teardown', async () => {
    const { hook } = setup({ resetDelayMs: 50, testFn: async () => ({ ok: true, message: 'done' }) })
    const done = hook.current.runTest()
    await vi.advanceTimersByTimeAsync(0)
    await done
    hook.flush()
    expect(hook.current.status).toBe('ok')
    hook.unmount()
    vi.advanceTimersByTime(5000)
    hook.flush()
    // No render fired above because dispose cancelled the setTimeout before it could
    // publish the idle reset on an already torn-down runner.
    expect(hook.current.status).toBe('ok')
  })
})
