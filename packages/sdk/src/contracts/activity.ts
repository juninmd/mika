/**
 * Activity signal — what Mika is doing right now, stated by herself. Replaces two desynced state
 * machines (main's ExpressionSyncService vs renderer's reconstructed appStatus), where `speaking`
 * emitted nothing at all. Channels are independent on purpose: she can talk and watch the screen
 * at once, so each channel reports its own state instead of one lossy scalar.
 */
export const ACTIVITY_CHANNELS = ['voice', 'vision'] as const
export type ActivityChannel = (typeof ACTIVITY_CHANNELS)[number]

/** Mirrors `VoiceState` from `ExpressionSyncService`. */
export const VOICE_ACTIVITY_STATES = [
  'idle',
  'listening',
  'thinking',
  'speaking',
  'acting',
  'confirming',
  'offline',
  'error',
] as const

/** `watching` fires on every capture cycle, not just ones that produce a comment — trust matters here. */
export const VISION_ACTIVITY_STATES = ['idle', 'watching', 'ocr'] as const

export type ActivityState = (typeof VOICE_ACTIVITY_STATES)[number] | (typeof VISION_ACTIVITY_STATES)[number]

export interface ActivityPayload {
  channel: ActivityChannel
  state: ActivityState
  /** Short context for the UI: observed app, OCR region, error reason. */
  detail?: string
  at: number
}
