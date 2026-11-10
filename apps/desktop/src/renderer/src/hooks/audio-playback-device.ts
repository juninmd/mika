// Output-device routing helpers (setSinkId + configured device lookup). No React import.
import { AUDIO_OUTPUT_DEVICE_KEY } from '../lib/audio-storage-keys'
import { rendererLog } from '../lib/renderer-logger'
import { readLocalStorage } from '../lib/safe-local-storage'

export function getConfiguredOutputDeviceId(): string {
  const saved = readLocalStorage(AUDIO_OUTPUT_DEVICE_KEY)
  return saved && saved !== 'default' ? saved : ''
}

type SinkIdCapable = {
  setSinkId?: (sinkId: string) => Promise<void>
}

let setSinkIdSupported = true

export async function applyOutputDevice(target: AudioContext | HTMLMediaElement, sinkId: string): Promise<void> {
  const sinkTarget = target as SinkIdCapable
  if (!sinkTarget.setSinkId) {
    if (setSinkIdSupported) {
      setSinkIdSupported = false
      rendererLog.warn('[AudioPlayback] setSinkId not supported — output device selection ignored')
    }
    return
  }
  try {
    await sinkTarget.setSinkId(sinkId)
  } catch (err) {
    rendererLog.warn('[AudioPlayback] Failed to route output to selected device:', err)
  }
}
