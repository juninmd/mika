import type { TranslationDict } from '@mika/sdk/i18n'
import type { AppStatus } from './app/app-shell.types'

export const SPEECH_STATUS_LABELS: Record<AppStatus, keyof TranslationDict> = {
  idle: 'shell.status.idle',
  listening: 'status.listening',
  thinking: 'shell.status.thinking',
  acting: 'shell.status.acting',
  confirming: 'shell.status.confirming',
  speaking: 'shell.status.speaking',
  offline: 'status.offline',
  error: 'shell.status.error',
}

export const SPEECH_STATUS_DOT: Record<AppStatus, string> = {
  idle: 'bg-[var(--neural-purple)] shadow-[0_0_18px_var(--neural-purple)]',
  listening: 'bg-ok shadow-[0_0_18px_var(--neural-cyan)]',
  thinking: 'bg-info shadow-[0_0_18px_var(--neural-cyan)]',
  acting: 'bg-info shadow-[0_0_18px_var(--neural-cyan)]',
  confirming: 'bg-warn shadow-[0_0_18px_var(--warn-soft)]',
  speaking: 'bg-[var(--neural-cyan)] shadow-[0_0_18px_var(--neural-cyan)]',
  offline: 'bg-surface-raise-strong shadow-none',
  error: 'bg-danger shadow-[0_0_18px_var(--danger-soft)]',
}
