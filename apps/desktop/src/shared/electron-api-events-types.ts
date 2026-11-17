import type { ActivityPayload, PowerProfileState } from '@mika/sdk/contracts'

/** `EventKey::BrainProactiveThought` as the backend publishes it (`hud_bridge.rs`). */
export type BrainProactiveThoughtPayload = {
  type?: string
  message?: string
  source?: string
  context?: string
  priority?: number
  /** Set by real-time reactions (`vision::media_reaction`). */
  media?: { kind: string; title: string }
}

export type EventsRuntimeApi = {
  onActivity: (callback: (payload: ActivityPayload) => void) => () => void
  onMoodChanged: (callback: (payload: { valence: number; arousal: number; label: string }) => void) => () => void

  onListeningCommand: (callback: (payload: { listening: boolean }) => void) => () => void

  onSensesPaused: (callback: (payload: { paused: boolean }) => void) => () => void
  onForceStopTts: (callback: (payload: { reason?: string }) => void) => () => void
  onInterruption: (
    callback: (payload: {
      transcript: string
      is_interruption?: boolean
      timestamp?: number
      window_label?: string
    }) => void,
  ) => () => void
  onNotification: (
    callback: (payload: { message: string; type?: 'info' | 'success' | 'warning' | 'error'; source?: string }) => void,
  ) => () => void
  onPersonCatalogUpdated: (callback: (payload: { personId: string; name?: string }) => void) => () => void
  onPersonCatalogDeleted: (callback: (payload: { personId: string }) => void) => () => void

  getPowerState: () => Promise<{ onBatteryPower: boolean } & Partial<PowerProfileState>>

  onPowerModeChanged: (callback: (payload: { onBatteryPower: boolean }) => void) => () => void
  /** O orçamento do governador de energia mudou (perfil, estado ou teto de fps). */
  onPowerProfile: (callback: (payload: PowerProfileState) => void) => () => void

  sendMicState: (enabled: boolean) => void

  onMicState: (callback: (payload: { enabled: boolean }) => void) => () => void
  onBrainProactiveThought: (callback: (payload: BrainProactiveThoughtPayload) => void) => () => void
  onChatError: (callback: (payload: { message: string; type?: string }) => void) => () => void
}
