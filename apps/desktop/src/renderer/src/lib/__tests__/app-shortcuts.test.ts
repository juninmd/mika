import { describe, expect, it } from 'vitest'
import { formatAccelerator, matchesAccelerator, shortcutById, WINDOW_SHORTCUTS } from '../app-shortcuts'

const key = (code: string, mods: Partial<Record<'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey', boolean>> = {}) => ({
  code,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
})

describe('app shortcuts catalog', () => {
  it('matches Ctrl or ⌘ for CommandOrControl', () => {
    expect(matchesAccelerator(key('KeyM', { ctrlKey: true }), 'CommandOrControl+M')).toBe(true)
    expect(matchesAccelerator(key('KeyM', { metaKey: true }), 'CommandOrControl+M')).toBe(true)
    expect(matchesAccelerator(key('KeyM'), 'CommandOrControl+M')).toBe(false)
  })

  // Ctrl+Shift+M is the global show/hide; the in-window Ctrl+M used to toggle the mic on it too.
  it('requires the exact modifier set', () => {
    expect(matchesAccelerator(key('KeyM', { ctrlKey: true, shiftKey: true }), 'CommandOrControl+M')).toBe(false)
    expect(matchesAccelerator(key('KeyH', { ctrlKey: true, shiftKey: true }), 'CommandOrControl+Shift+H')).toBe(true)
    expect(matchesAccelerator(key('Escape'), 'Escape')).toBe(true)
    expect(matchesAccelerator(key('F9'), 'F9')).toBe(true)
  })

  it('formats per platform', () => {
    expect(formatAccelerator('CommandOrControl+Shift+M', false)).toBe('Ctrl+Shift+M')
    expect(formatAccelerator('CommandOrControl+Space', true)).toBe('⌘+Space')
    expect(formatAccelerator('Escape', false)).toBe('Esc')
  })

  // Two in-window handlers on one combo fire both actions (Ctrl+K opened the palette AND the input).
  it('never binds two in-window actions to the same combo', () => {
    const accelerators = WINDOW_SHORTCUTS.map((s) => s.accelerator)
    expect(new Set(accelerators).size).toBe(accelerators.length)
  })

  // F11 is the window's, never OS-global: a global F11 would swallow fullscreen in the user's game or browser.
  it('F11 toggles fullscreen in-window only', () => {
    const fullscreen = shortcutById('fullscreen')
    expect(fullscreen.accelerator).toBe('F11')
    expect(WINDOW_SHORTCUTS).toContain(fullscreen)
    expect(matchesAccelerator(key('F11'), fullscreen.accelerator)).toBe(true)
  })
})
