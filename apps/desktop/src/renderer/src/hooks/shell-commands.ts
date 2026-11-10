import type { TranslationDict } from '@mika/sdk/i18n'
import { t } from '@mika/sdk/i18n'
import { escapeRegExp } from '@mika/sdk/utils/regex'
import type { AppMessage, AppStatus } from '../components/app/app-shell.types'
import { downloadBlobAs } from '../lib/download-blob'
import { MATCH_EFFECT_IDS, type MatchEffectId } from '../lib/match-effect'

export const MIN_TRANSCRIPT_CHARS = 2

// Shared by /help and the autocomplete dropdown so the two cannot drift.
export type SlashCommandDef = { command: string; descriptionKey: keyof TranslationDict }

export const SLASH_COMMAND_DEFS: SlashCommandDef[] = [
  { command: 'clear', descriptionKey: 'shell.composer.commands.clear' },
  { command: 'limpar', descriptionKey: 'shell.composer.commands.clear' },
  { command: 'help', descriptionKey: 'shell.composer.commands.help' },
  { command: 'ajuda', descriptionKey: 'shell.composer.commands.help' },
  { command: 'stats', descriptionKey: 'shell.composer.commands.stats' },
  { command: 'estatisticas', descriptionKey: 'shell.composer.commands.stats' },
  { command: 'export', descriptionKey: 'shell.composer.commands.export' },
  { command: 'exportar', descriptionKey: 'shell.composer.commands.export' },
  ...MATCH_EFFECT_IDS.map((id) => ({ command: id, descriptionKey: 'shell.composer.commands.effect' as const })),
]

// 'network' intentionally excluded: transient network timeouts self-resolve, so marking it fatal
// would disable the mic permanently for a failure that fixes itself next turn.
const FATAL_SPEECH_ERRORS = [
  'not-allowed',
  'NotAllowedError',
  'audio-capture',
  'browser-stt-unavailable',
  'media-recorder-unavailable',
  'media-devices-unavailable',
  'OverconstrainedError',
  'NotFoundError',
  'offline-mode-blocks-browser-stt',
]

// Whole-token match, not loose substring — a free-text exception message can't be confused with an exact error code.
const FATAL_SPEECH_ERROR_PATTERNS = FATAL_SPEECH_ERRORS.map((token) => new RegExp(`\\b${escapeRegExp(token)}\\b`))

export function isFatalSpeechError(error: string): boolean {
  return FATAL_SPEECH_ERROR_PATTERNS.some((pattern) => pattern.test(error))
}

const SPEECH_ERROR_MESSAGE_KEYS = {
  'not-allowed': 'shell.composer.micDenied',
  NotAllowedError: 'shell.composer.micDenied',
  NotFoundError: 'shell.composer.micNotFound',
  OverconstrainedError: 'shell.composer.micNotFound',
  'audio-capture': 'shell.composer.micNotFound',
  'offline-mode-blocks-browser-stt': 'shell.voice.offlineBlocksBrowserStt',
  'browser-stt-unavailable-in-electron': 'shell.voice.browserSttUnavailable',
} as const satisfies Record<string, keyof TranslationDict>

/** Dedicated localized message for a known speech failure code; `undefined` → caller shows the raw detail. */
export function speechErrorMessageKey(error: string): keyof TranslationDict | undefined {
  return (SPEECH_ERROR_MESSAGE_KEYS as Record<string, keyof TranslationDict>)[error]
}

// A destructive /clear must be sent twice: a stray Enter or history recall can't wipe the chat, and
// there is no blocking native dialog (window.confirm does not render in the Tauri WebView).
export const CLEAR_CONFIRM_WINDOW_MS = 10_000

export type ClearGuard = { armedAt: number | null }

export function isClearConfirmed(guard: ClearGuard, now: number): boolean {
  return guard.armedAt !== null && now - guard.armedAt <= CLEAR_CONFIRM_WINDOW_MS
}

// Built from SLASH_COMMAND_DEFS so /help lists exactly what autocomplete offers, in the active locale.
export function buildHelpText(): string {
  const seen = new Set<string>(['shell.composer.commands.effect'])
  const lines: string[] = []
  for (const def of SLASH_COMMAND_DEFS) {
    if (seen.has(def.descriptionKey)) continue
    seen.add(def.descriptionKey)
    lines.push(`/${def.command} — ${t(def.descriptionKey)}`)
  }
  lines.push(`${MATCH_EFFECT_IDS.map((id) => `/${id}`).join(', ')} — ${t('shell.composer.commands.effect')}`)
  return lines.join('\n')
}

type HandleSlashCommandParams = {
  text: string
  messages: AppMessage[]
  clearGuard: ClearGuard
  now?: number
  doClearHistory: () => Promise<void> | void
  setBubbleVisible: (v: boolean) => void
  addNotification: (type: 'info' | 'success' | 'warning' | 'error', message: string, title?: string) => void
  setAppStatus: (s: AppStatus) => void

  onEffectCommand?: (effect: MatchEffectId) => void
}

const EFFECT_SLASH = new Set<string>(MATCH_EFFECT_IDS)

export function handleSlashCommand({
  text,
  messages,
  doClearHistory,
  setBubbleVisible,
  addNotification,
  setAppStatus,
  onEffectCommand,
  clearGuard,
  now = Date.now(),
}: HandleSlashCommandParams): boolean {
  const [cmd] = text.slice(1).split(/\s+/)
  const lower = cmd.toLowerCase()

  if (EFFECT_SLASH.has(lower)) {
    onEffectCommand?.(lower as MatchEffectId)
    setAppStatus('idle')
    return true
  }

  if (lower === 'clear' || lower === 'limpar') {
    if (!isClearConfirmed(clearGuard, now)) {
      clearGuard.armedAt = now
      addNotification('warning', t('shell.commands.clearArmed'), t('shell.commands.title'))
      setAppStatus('idle')
      return true
    }
    clearGuard.armedAt = null
    void doClearHistory()
    setBubbleVisible(false)
    addNotification('success', t('shell.commands.cleared'), t('shell.commands.title'))
    setAppStatus('idle')
    return true
  }

  if (lower === 'help' || lower === 'ajuda') {
    addNotification('info', buildHelpText(), t('shell.composer.commands.title'))
    setAppStatus('idle')
    return true
  }

  if (lower === 'stats' || lower === 'estatisticas') {
    const total = messages.length
    const user = messages.filter((m) => m.role === 'user').length
    const mikaCount = total - user
    const body = t('shell.commands.statsBody')
      .replace('{{total}}', String(total))
      .replace('{{user}}', String(user))
      .replace('{{mika}}', String(mikaCount))
    addNotification('info', body, t('shell.commands.statsTitle'))
    setAppStatus('idle')
    return true
  }

  if (lower === 'export' || lower === 'exportar') {
    const blob = new Blob([JSON.stringify(messages, null, 2)], { type: 'application/json' })
    downloadBlobAs(blob, `mika-chat-${Date.now()}.json`)
    addNotification('success', t('shell.commands.exported'), t('shell.commands.title'))
    setAppStatus('idle')
    return true
  }

  return false
}
