import type { AppStatus } from '../components/app/app-shell.types'

// `null` means no change, so the caller skips a no-op `setState`.

/** TTS stopped playing: drop 'speaking' back to 'idle'. Any other status is left alone. */
export function nextStatusOnTtsStop(ttsIsPlaying: boolean, appStatus: AppStatus): AppStatus | null {
  if (ttsIsPlaying) return null
  return appStatus === 'speaking' ? 'idle' : null
}

/** Offline mode forces 'offline'; leaving it only clears the 'offline' status, never any other. */
export function nextStatusOnOfflineChange(offlineMode: boolean, appStatus: AppStatus): AppStatus | null {
  if (offlineMode) return 'offline'
  return appStatus === 'offline' ? 'idle' : null
}

/** VAD + mic drive 'listening' only from/to 'idle' — never interrupts 'thinking', 'speaking', etc. */
export function nextStatusOnVad(vadIsSpeaking: boolean, micEnabled: boolean, appStatus: AppStatus): AppStatus | null {
  if (vadIsSpeaking && micEnabled && appStatus === 'idle') return 'listening'
  if ((!vadIsSpeaking || !micEnabled) && appStatus === 'listening') return 'idle'
  return null
}
