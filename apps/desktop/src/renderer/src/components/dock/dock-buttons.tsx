import type { TranslationDict } from '@mika/sdk/i18n'
import { BrainIcon, HistoryIcon, MessageIcon, MicrophoneIcon, SettingsIcon, StopIcon } from '../Icons'

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

export const DOCK_ICONS: Record<DockKey, () => React.ReactNode> = {
  stop: () => <StopIcon className="h-3.5 w-3.5" aria-hidden="true" />,
  mic: () => <MicrophoneIcon className="h-4 w-4" aria-hidden="true" />,
  chat: () => <MessageIcon className="h-4 w-4" aria-hidden="true" />,
  history: () => <HistoryIcon className="h-4 w-4" aria-hidden="true" />,
  wheel: () => <BrainIcon className="h-4 w-4" aria-hidden="true" />,
  settings: () => <SettingsIcon className="h-4 w-4" aria-hidden="true" />,
}
