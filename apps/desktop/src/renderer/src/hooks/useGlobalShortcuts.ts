import { useEffect } from 'react'
import { matchesAccelerator, shortcutById } from '../lib/app-shortcuts'
import { getElectronApi } from '../lib/shell-api'

interface GlobalShortcutsOptions {
  onToggleInput: () => void
  onToggleMic: () => void
  onToggleHistory?: () => void
  onToggleFullscreen?: () => void
  onHide?: () => void
  /** Hold-to-talk (F8, OS-level). Only passed in push-to-talk mode — in continuous mode a release would cut the open mic. */
  onPushToTalk?: (pressed: boolean) => void
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  const tag = target.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT'
}

// Accelerators come from `app-shortcuts.ts` — the same list the "Atalhos" card shows. Ctrl+K is
// the command palette's (`CommandPalette.tsx`); binding it here too opened palette AND input.
function windowBindings(opts: GlobalShortcutsOptions, hasShell: boolean): Array<[string, (() => void) | undefined]> {
  return [
    // Ctrl+Space is OS-global under the shell; in-window only as the no-shell fallback.
    [shortcutById('toggleInput').accelerator, hasShell ? undefined : opts.onToggleInput],
    [shortcutById('history').accelerator, opts.onToggleHistory],
    [shortcutById('windowMic').accelerator, opts.onToggleMic],
    [shortcutById('fullscreen').accelerator, opts.onToggleFullscreen],
    [shortcutById('close').accelerator, opts.onHide],
  ]
}

function handleKey(e: KeyboardEvent, opts: GlobalShortcutsOptions, hasShell: boolean): void {
  if (isEditableTarget(e.target)) {
    if (e.code === 'Escape') opts.onHide?.()
    return
  }
  for (const [accelerator, action] of windowBindings(opts, hasShell)) {
    if (!action || !matchesAccelerator(e, accelerator)) continue
    e.preventDefault()
    action()
    return
  }
}

function shellApi(): ReturnType<typeof getElectronApi> | null {
  try {
    return getElectronApi()
  } catch {
    return null
  }
}

export function useGlobalShortcuts(opts: GlobalShortcutsOptions) {
  const { onToggleInput, onToggleMic, onToggleHistory, onToggleFullscreen, onHide, onPushToTalk } = opts

  useEffect(() => {
    const hasShell = Boolean(shellApi()?.shell.shortcuts)
    const current = { onToggleInput, onToggleMic, onToggleHistory, onToggleFullscreen, onHide }
    const listener = (e: KeyboardEvent) => handleKey(e, current, hasShell)
    window.addEventListener('keydown', listener)
    return () => window.removeEventListener('keydown', listener)
  }, [onToggleHistory, onToggleInput, onToggleMic, onToggleFullscreen, onHide])

  useEffect(() => {
    const shortcuts = shellApi()?.shell.shortcuts
    if (!shortcuts) return
    const unsubs = [shortcuts.onToggleInput(() => onToggleInput()), shortcuts.onToggleMic(() => onToggleMic())]
    if (onPushToTalk) unsubs.push(shortcuts.onPushToTalk((pressed) => onPushToTalk(pressed)))
    return () => {
      for (const unsub of unsubs) unsub()
    }
  }, [onToggleInput, onToggleMic, onPushToTalk])
}
