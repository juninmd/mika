import type { useI18n } from '../hooks/useI18n'
import { downloadBlobAs } from '../lib/download-blob'
import type { Message } from './chat-history-components'

export const FETCH_TIMEOUT_MS = 5000

export function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error('timeout')), ms)
  })
  return Promise.race([promise, deadline]).finally(() => clearTimeout(timer))
}

export function toHistoryMessages(
  messages: Array<{ id: string; role: string; content: string; timestamp: number }>,
): Message[] {
  return messages.filter(
    (message): message is Message =>
      message.role === 'user' || message.role === 'assistant' || message.role === 'system',
  )
}

export function downloadBlob(blob: Blob, extension: string) {
  downloadBlobAs(blob, `mika-chat-${Date.now()}.${extension}`)
}

export function historyToMarkdown(history: Message[], t: ReturnType<typeof useI18n>['t'], locale: string): string {
  const lines = history.map((m) => {
    const role = m.role === 'user' ? `**${t('shell.history.roleUser')}**` : '**Mika**'
    const date = new Date(m.timestamp).toLocaleString(locale)
    return `### ${role} _( ${date} )_\n${m.content}\n`
  })
  const md = `# ${t('shell.history.title')} - Mika\n\n${lines.join('---\n\n')}`
  return md
}
