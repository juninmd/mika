import type { TranslationDict } from '@mika/sdk/i18n'
import { BrainCircuit, History, MessageCircle, Mic, Settings2, Square } from 'lucide-react'

export type DockKey = 'stop' | 'mic' | 'chat' | 'history' | 'wheel' | 'settings'

/** Voice · conversation · tools · system — separators between groups, never between siblings. */
export const DOCK_GROUPS: ReadonlyArray<readonly DockKey[]> = [
  ['stop', 'mic'],
  ['chat', 'history'],
  ['wheel'],
  ['settings'],
]

export const DOCK_SHORT_LABEL: Record<DockKey, keyof TranslationDict> = {
  stop: 'shell.dock.short.stop',
  mic: 'shell.dock.short.mic',
  chat: 'shell.dock.short.chat',
  history: 'shell.dock.short.history',
  wheel: 'shell.dock.short.wheel',
  settings: 'shell.dock.short.settings',
}

/** `useNeuralWheel` binds ctrlKey+Shift+Space on every OS (not Cmd on macOS), so it is not an accelerator of the catalog. */
export const WHEEL_KEY = 'Ctrl+Shift+Space'

/** Lucide, the same set as the wheel and settings: one stroke weight across every Mika surface. */
export const DOCK_ICONS: Record<DockKey, () => React.ReactNode> = {
  stop: () => <Square size={14} strokeWidth={2.4} fill="currentColor" aria-hidden="true" />,
  mic: () => <Mic size={17} strokeWidth={1.8} aria-hidden="true" />,
  chat: () => <MessageCircle size={17} strokeWidth={1.8} aria-hidden="true" />,
  history: () => <History size={17} strokeWidth={1.8} aria-hidden="true" />,
  wheel: () => <BrainCircuit size={17} strokeWidth={1.8} aria-hidden="true" />,
  settings: () => <Settings2 size={17} strokeWidth={1.8} aria-hidden="true" />,
}
