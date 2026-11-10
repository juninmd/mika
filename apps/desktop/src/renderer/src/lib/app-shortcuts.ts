import type { TranslationDict } from '@mika/sdk/i18n'

/**
 * Every keyboard shortcut the app really answers to — the one list the "Atalhos" card, the wizard
 * and `useGlobalShortcuts` read. `GLOBAL_SHORTCUTS` are OS-level (`src-tauri/src/shortcuts.rs`
 * registers them; a Rust test reads this block and fails if the two sides diverge).
 * Accelerators use the Tauri syntax on both scopes.
 */
export type AppShortcut = {
  id: string
  accelerator: string
  /** Held, not pressed: the action lasts while the key is down. */
  hold?: boolean
  label: keyof TranslationDict
  description: keyof TranslationDict
}

export const GLOBAL_SHORTCUTS: readonly AppShortcut[] = [
  {
    id: 'toggleMic',
    accelerator: 'F9',
    label: 'settings.shortcuts.toggleMic.label',
    description: 'settings.shortcuts.toggleMic.description',
  },
  {
    id: 'pushToTalk',
    accelerator: 'F8',
    hold: true,
    label: 'settings.shortcuts.pushToTalk.label',
    description: 'settings.shortcuts.pushToTalk.description',
  },
  {
    id: 'toggleInput',
    accelerator: 'CommandOrControl+Space',
    label: 'settings.shortcuts.toggleInput.label',
    description: 'settings.shortcuts.toggleInput.description',
  },
  {
    id: 'toggleVisibility',
    accelerator: 'CommandOrControl+Shift+M',
    label: 'settings.shortcuts.toggleVisibility.label',
    description: 'settings.shortcuts.toggleVisibility.description',
  },
  {
    id: 'muteSpeech',
    accelerator: 'CommandOrControl+Shift+S',
    label: 'settings.shortcuts.muteSpeech.label',
    description: 'settings.shortcuts.muteSpeech.description',
  },
  {
    id: 'visionScreenshot',
    accelerator: 'CommandOrControl+Shift+T',
    label: 'settings.shortcuts.visionScreenshot.label',
    description: 'settings.shortcuts.visionScreenshot.description',
  },
]

export const WINDOW_SHORTCUTS: readonly AppShortcut[] = [
  {
    id: 'windowMic',
    accelerator: 'CommandOrControl+M',
    label: 'settings.shortcuts.windowMic.label',
    description: 'settings.shortcuts.windowMic.description',
  },
  {
    id: 'palette',
    accelerator: 'CommandOrControl+K',
    label: 'settings.shortcuts.palette.label',
    description: 'settings.shortcuts.palette.description',
  },
  {
    id: 'history',
    accelerator: 'CommandOrControl+Shift+H',
    label: 'settings.shortcuts.history.label',
    description: 'settings.shortcuts.history.description',
  },
  {
    id: 'fullscreen',
    accelerator: 'F11',
    label: 'settings.shortcuts.fullscreen.label',
    description: 'settings.shortcuts.fullscreen.description',
  },
  {
    id: 'close',
    accelerator: 'Escape',
    label: 'settings.shortcuts.close.label',
    description: 'settings.shortcuts.close.description',
  },
]

export function shortcutById(id: string): AppShortcut {
  const found = [...GLOBAL_SHORTCUTS, ...WINDOW_SHORTCUTS].find((s) => s.id === id)
  if (!found) throw new Error(`unknown shortcut ${id}`)
  return found
}

type KeyLike = Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>

function codeOf(token: string): string {
  return /^[A-Z]$/.test(token) ? `Key${token}` : token
}

export function matchesAccelerator(e: KeyLike, accelerator: string): boolean {
  const tokens = accelerator.split('+')
  const main = tokens[tokens.length - 1] ?? ''
  const wantsCmd = tokens.includes('CommandOrControl')
  return (
    e.code === codeOf(main) &&
    (e.ctrlKey || e.metaKey) === wantsCmd &&
    e.shiftKey === tokens.includes('Shift') &&
    e.altKey === tokens.includes('Alt')
  )
}

export function formatAccelerator(accelerator: string, mac: boolean): string {
  return accelerator
    .split('+')
    .map((token) => {
      if (token === 'CommandOrControl') return mac ? '⌘' : 'Ctrl'
      if (token === 'Escape') return 'Esc'
      return token
    })
    .join('+')
}
