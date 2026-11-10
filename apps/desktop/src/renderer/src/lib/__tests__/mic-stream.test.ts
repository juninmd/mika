import { describe, expect, it, vi } from 'vitest'
import { createGenerationGuard, openMicIfCurrent, stopStream } from '../mic-stream'

function fakeStream() {
  const stop = vi.fn()
  return { stream: { getTracks: () => [{ stop }, { stop }] } as unknown as MediaStream, stop }
}

describe('createGenerationGuard', () => {
  it('only the latest begin() is current; invalidate() retires everything in flight', () => {
    const guard = createGenerationGuard()
    const first = guard.begin()
    const second = guard.begin()
    expect(guard.isCurrent(first)).toBe(false)
    expect(guard.isCurrent(second)).toBe(true)
    guard.invalidate()
    expect(guard.isCurrent(second)).toBe(false)
  })
})

describe('openMicIfCurrent', () => {
  // Regression: a stop()/unmount that landed while getUserMedia was pending was ignored, so the stream
  // that resolved afterwards was kept open with nobody left to close it (mic LED stuck on).
  it('hands the stream over when the session is still current', async () => {
    const { stream, stop } = fakeStream()
    await expect(
      openMicIfCurrent(
        async () => stream,
        () => true,
      ),
    ).resolves.toBe(stream)
    expect(stop).not.toHaveBeenCalled()
  })

  it('stops every track and returns null when the session went stale while awaiting', async () => {
    const { stream, stop } = fakeStream()
    let current = true
    const open = async () => {
      current = false
      return stream
    }
    await expect(openMicIfCurrent(open, () => current)).resolves.toBeNull()
    expect(stop).toHaveBeenCalledTimes(2)
  })

  it('propagates a getUserMedia rejection untouched', async () => {
    await expect(
      openMicIfCurrent(
        () => Promise.reject(new Error('denied')),
        () => true,
      ),
    ).rejects.toThrow('denied')
  })
})

describe('stopStream', () => {
  it('stops all tracks and tolerates null', () => {
    const { stream, stop } = fakeStream()
    stopStream(stream)
    stopStream(null)
    expect(stop).toHaveBeenCalledTimes(2)
  })
})
