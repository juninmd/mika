type ShortcutEvent = Pick<KeyboardEvent, 'ctrlKey' | 'metaKey'> & { key?: string }

export function isMacPlatform(
  userAgent: string = typeof navigator === 'undefined' ? '' : navigator.userAgent,
): boolean {
  return /Mac|iPhone|iPad/.test(userAgent)
}

// `key` is 'K' with Caps Lock or Shift held, and Chromium sends it undefined on autofill picks.
export function isPaletteShortcut(event: ShortcutEvent): boolean {
  return (event.ctrlKey || event.metaKey) && event.key?.toLowerCase() === 'k'
}

export function modifierKeyLabel(mac: boolean = isMacPlatform()): string {
  return mac ? '⌘' : 'Ctrl'
}
