import { describe, expect, it } from 'vitest'
import { miniModeAction, miniOverlayOpen, parseMiniModeStyle } from '../mini-mode'

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

describe('parseMiniModeStyle', () => {
  it('keeps the two dock sides and falls back to the head square for anything else', () => {
    expect(parseMiniModeStyle('dock-right')).toBe('dock-right')
    expect(parseMiniModeStyle('dock-left')).toBe('dock-left')
    expect(parseMiniModeStyle('dock-top')).toBe('head')
    expect(parseMiniModeStyle(undefined)).toBe('head')
  })
})

// The edge dock has its own compact chat: opening it must not throw her back to the full window.
describe('miniOverlayOpen', () => {
  const closed = { style: 'head', showInput: false, showHistory: false, wheelOpen: false } as const
  it('counts the chat as needing room only in the head square', () => {
    expect(miniOverlayOpen({ ...closed, showInput: true })).toBe(true)
    expect(miniOverlayOpen({ ...closed, style: 'dock-right', showInput: true })).toBe(false)
  })
  it('still counts history and the wheel in the dock', () => {
    expect(miniOverlayOpen({ ...closed, style: 'dock-left', showHistory: true })).toBe(true)
    expect(miniOverlayOpen({ ...closed, style: 'dock-left', wheelOpen: true })).toBe(true)
  })
})
