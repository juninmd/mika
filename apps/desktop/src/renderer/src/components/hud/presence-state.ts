import type { TranslationDict } from '@mika/sdk/i18n'
import type { AppStatus } from '../app/app-shell.types'

/** One word for "what is Mika doing right now", derived from the signals the shell already has. */
export type PresenceKind =
  | 'error'
  | 'confirming'
  | 'speaking'
  | 'thinking'
  | 'offline'
  | 'held'
  | 'hearing'
  | 'ready'
  | 'muted'

export type PresenceInput = {
  appStatus: AppStatus
  /** The LLM provider health check failed: she can listen but cannot answer. */
  providerDown: boolean
  micEnabled: boolean
  /** VAD gate open: voice, not room noise. */
  hearing: boolean
  /** Speech heard during a running turn, queued for the next one. */
  speechHeld: boolean
  ttsIsPlaying: boolean
  gaming: boolean
}

export type Presence = {
  kind: PresenceKind
  /** Semantic color token (a `var(--…)` of the theme) — the orb never carries a literal. */
  tone: string
  /** Resting states (`ready`/`muted`) are inactive: transparent mode shrinks them to a dot. */
  active: boolean
  gaming: boolean
  /** Which live level drives the ring: the microphone, the TTS output, or nothing. */
  meter: 'mic' | 'tts' | 'none'
  labelKey: keyof TranslationDict
}

const TONE: Record<PresenceKind, string> = {
  error: 'var(--danger)',
  confirming: 'var(--warn)',
  speaking: 'var(--info)',
  thinking: 'var(--accent)',
  offline: 'var(--text-subtle)',
  held: 'var(--warn)',
  hearing: 'var(--ok)',
  ready: 'var(--ok)',
  muted: 'var(--text-muted)',
}

function presenceKind(i: PresenceInput): PresenceKind {
  if (i.appStatus === 'error') return 'error'
  if (i.appStatus === 'confirming') return 'confirming'
  if (i.appStatus === 'speaking' || i.ttsIsPlaying) return 'speaking'
  if (i.appStatus === 'thinking' || i.appStatus === 'acting') return 'thinking'
  if (i.appStatus === 'offline' || i.providerDown) return 'offline'
  if (!i.micEnabled) return 'muted'
  if (i.hearing) return i.speechHeld ? 'held' : 'hearing'
  return 'ready'
}

function meterFor(kind: PresenceKind): Presence['meter'] {
  if (kind === 'speaking') return 'tts'
  return kind === 'hearing' || kind === 'held' || kind === 'ready' ? 'mic' : 'none'
}

export function derivePresence(input: PresenceInput): Presence {
  const kind = presenceKind(input)
  return {
    kind,
    tone: TONE[kind],
    active: kind !== 'ready' && kind !== 'muted',
    gaming: input.gaming,
    meter: meterFor(kind),
    labelKey: `shell.presence.${kind}` as keyof TranslationDict,
  }
}
