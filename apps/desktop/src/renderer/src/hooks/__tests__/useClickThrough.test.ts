import { describe, expect, it, vi } from 'vitest'
import { pollIgnoreDecision, releaseClickThrough } from '../useClickThrough'

const overAvatar = () => false
const overTransparent = () => true

describe('useClickThrough — pollIgnoreDecision', () => {
  // Regression: this path only ever re-enabled clicks in the dock strip, so once click-through
  // turned on (any blur did it) the avatar itself could never be clicked or dragged again.
  it('turns click-through off when the polled cursor is back over the avatar', () => {
    expect(pollIgnoreDecision({ isInside: true, insideX: 200, insideY: 300 }, 640, false, overAvatar)).toBe(false)
  })

  it('keeps click-through on over transparent pixels', () => {
    expect(pollIgnoreDecision({ isInside: true, insideX: 5, insideY: 5 }, 640, false, overTransparent)).toBe(true)
  })

  it('always accepts clicks in the dock strip at the bottom', () => {
    expect(pollIgnoreDecision({ isInside: true, insideX: 5, insideY: 600 }, 640, false, overTransparent)).toBe(false)
  })

  it('leaves the state alone outside the window or mid-drag', () => {
    expect(pollIgnoreDecision({ isInside: false, insideX: -5, insideY: 300 }, 640, false, overAvatar)).toBeNull()
    expect(pollIgnoreDecision({ isInside: true, insideX: 200, insideY: 300 }, 640, true, overTransparent)).toBeNull()
  })
})

describe('useClickThrough — releaseClickThrough', () => {
  // Regression: the hook's cleanup only dropped listeners, so a shell that unmounted (error boundary,
  // crash) while the window was ignoring the mouse left the whole window un-clickable for good.
  it('hands the mouse back to the window', () => {
    const setIgnoreMouseEvents = vi.fn()
    releaseClickThrough({ setIgnoreMouseEvents })
    expect(setIgnoreMouseEvents).toHaveBeenCalledExactlyOnceWith(false)
  })

  it('never throws out of an unmount even if the bridge is already gone', () => {
    const setIgnoreMouseEvents = vi.fn(() => {
      throw new Error('window closed')
    })
    expect(() => releaseClickThrough({ setIgnoreMouseEvents })).not.toThrow()
  })
})
