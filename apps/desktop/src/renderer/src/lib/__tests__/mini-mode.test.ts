import { describe, expect, it } from 'vitest'
import { miniModeAction } from '../mini-mode'

const quiet = { mini: true, entered: true, blocking: false, overlayOpen: false }

describe('miniModeAction', () => {
  it('does nothing outside mini mode, whatever is open', () => {
    expect(miniModeAction({ ...quiet, mini: false, overlayOpen: true, blocking: true })).toBeNull()
  })

  // Turning mini on with the chat open must shrink her, not bounce straight back to the full window.
  it('closes what was open on the way in instead of leaving again', () => {
    expect(miniModeAction({ ...quiet, entered: false, overlayOpen: true })).toBe('close-overlays')
  })

  it('leaves when the chat, history or wheel is summoned while already mini', () => {
    expect(miniModeAction(quiet)).toBeNull()
    expect(miniModeAction({ ...quiet, overlayOpen: true })).toBe('leave')
  })

  // An approval or a legal/crisis notice cannot be dismissed for her: it wins even on the way in.
  it('always leaves for something the user has to read or answer', () => {
    expect(miniModeAction({ ...quiet, blocking: true })).toBe('leave')
    expect(miniModeAction({ ...quiet, entered: false, blocking: true })).toBe('leave')
  })
})
