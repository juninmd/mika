import { t } from '@mika/sdk/i18n'
import { isNetworkError } from '@mika/sdk-ui'
import type { AppMessage } from '../components/app/app-shell.types'

/** Shortens an error for the notification bubble; network errors get a friendlier fixed copy. */
export function toShortErrorMessage(message: string): string {
  if (isNetworkError(message)) return t('shell.error.network')
  return message.length > 120 ? t('shell.error.sendFailed') : message
}

// A retried IPC round-trip must not double the bubble.
export function appendAssistantMessage(prev: AppMessage[], response: { id?: string; content: string }): AppMessage[] {
  const last = prev[prev.length - 1]
  if (last?.role === 'assistant' && last.content === response.content) return prev
  return [
    ...prev,
    {
      id: response.id ?? crypto.randomUUID(),
      role: 'assistant' as const,
      content: response.content,
      timestamp: Date.now(),
    },
  ]
}

/** `chat-send` answers a failed turn as `role: 'system'` with the error text as `content` — not a reply of hers. */
export function isFailedTurn(response: { role?: string }): boolean {
  return response.role === 'system'
}
