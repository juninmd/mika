import { describe, expect, it } from 'vitest'
import { isMacPlatform, isPaletteShortcut, modifierKeyLabel } from '../keyboard-shortcuts'

describe('keyboard shortcuts', () => {
  it('shows Ctrl outside macOS, where the handler listens to Ctrl and there is no ⌘ key', () => {
    const windows = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/129.0'
    expect(isMacPlatform(windows)).toBe(false)
    expect(modifierKeyLabel(false)).toBe('Ctrl')
  })

  it('shows ⌘ on macOS', () => {
    expect(isMacPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15')).toBe(true)
    expect(modifierKeyLabel(true)).toBe('⌘')
  })

  it('opens the palette with Caps Lock on (key arrives as uppercase K)', () => {
    expect(isPaletteShortcut({ ctrlKey: true, metaKey: false, key: 'K' })).toBe(true)
    expect(isPaletteShortcut({ ctrlKey: false, metaKey: true, key: 'k' })).toBe(true)
  })

  it('ignores K without a modifier and other letters with one', () => {
    expect(isPaletteShortcut({ ctrlKey: false, metaKey: false, key: 'k' })).toBe(false)
    expect(isPaletteShortcut({ ctrlKey: true, metaKey: false, key: 'j' })).toBe(false)
  })

  // A throw here would escape the window keydown listener on every autofilled input.
  it('does not throw when Chromium sends keydown without a key (autofill pick)', () => {
    expect(isPaletteShortcut({ ctrlKey: true, metaKey: false, key: undefined })).toBe(false)
  })
})
