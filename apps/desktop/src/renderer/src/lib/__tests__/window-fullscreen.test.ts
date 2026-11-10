import { describe, expect, it, vi } from 'vitest'
import { type FullscreenWindowApi, subscribeFullscreen } from '../window-fullscreen'

function fakeApi(initial: boolean) {
  let listener: ((payload: { fullscreen: boolean }) => void) | undefined
  const off = vi.fn()
  const api: FullscreenWindowApi = {
    isFullscreen: vi.fn().mockResolvedValue(initial),
    onFullscreenChange: (cb) => {
      listener = cb
      return off
    },
  }
  return { api, off, emit: (fullscreen: boolean) => listener?.({ fullscreen }) }
}

describe('subscribeFullscreen', () => {
  it('reports the window state read from the backend, then every pushed change', async () => {
    const { api, emit } = fakeApi(false)
    const seen: boolean[] = []
    subscribeFullscreen(api, (value) => seen.push(value))
    await vi.waitFor(() => expect(seen).toEqual([false]))
    emit(true)
    emit(false)
    expect(seen).toEqual([false, true, false])
  })

  // The pushed event is newer than the one-shot read that was still in flight when it landed.
  it('does not let a late initial read overwrite a pushed change', async () => {
    const { api, emit } = fakeApi(false)
    const seen: boolean[] = []
    subscribeFullscreen(api, (value) => seen.push(value))
    emit(true)
    await Promise.resolve()
    await Promise.resolve()
    expect(seen).toEqual([true])
  })

  it('stops listening and ignores the pending read after unsubscribe', async () => {
    const { api, off } = fakeApi(true)
    const seen: boolean[] = []
    const unsubscribe = subscribeFullscreen(api, (value) => seen.push(value))
    unsubscribe()
    await Promise.resolve()
    await Promise.resolve()
    expect(off).toHaveBeenCalledOnce()
    expect(seen).toEqual([])
  })

  it('degrades to "unknown" silently when the bridge has no fullscreen support', () => {
    const notify = vi.fn()
    expect(() => subscribeFullscreen({}, notify)()).not.toThrow()
    expect(notify).not.toHaveBeenCalled()
  })
})
