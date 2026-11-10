export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

export type LogAudience = 'human' | 'ai'

export interface LogItem {
  timestamp: string

  at: number
  level: LogLevel
  scope: string
  msg: string
  meta?: Record<string, unknown>
  formatted: string
  audience: LogAudience

  traceId?: string
  /** Consecutive identical lines collapsed by the backend ring (`log_buffer.rs`); absent means 1. */
  repeatCount?: number
}

export const LEVEL_META: Record<LogLevel, { emoji: string; method: 'debug' | 'info' | 'warn' | 'error' }> = {
  debug: { emoji: '🐛', method: 'debug' },
  info: { emoji: 'ℹ️ ', method: 'info' },
  warn: { emoji: '⚠️ ', method: 'warn' },
  error: { emoji: '❌', method: 'error' },
}

const SCOPE_EMOJIS: Array<[string[], string]> = [
  [['chat'], '💬'],
  [['tool'], '🔧'],
  [['brain', 'llm'], '🧠'],
  [['voice', 'tts', 'stt'], '🎙️ '],
  [['vision'], '👁️ '],
  [['event', 'eventbus'], '🌐'],
  [['ipc'], '🔗'],
  [['store', 'config'], '📦'],
  [['system', 'core'], '⚙️ '],
]

export function scopeEmoji(scope: string): string {
  const s = scope.toLowerCase()
  for (const [prefixes, emoji] of SCOPE_EMOJIS) {
    if (prefixes.some((p) => s === p || s.startsWith(p))) return emoji
  }
  return '🔌'
}

export function ts(): string {
  const d = new Date()
  const pad = (n: number, l = 2) => n.toString().padStart(l, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`
}
