// Barge-in VAD helpers: amplitude math and worklet message decisions. No React import.
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

export const VAD_WORKLET_PATH = () => new URL('vad-processor.worklet.js', window.location.href).href

export function logVadSetupFailure(err: unknown, cancelled: boolean): void {
  if (cancelled && err instanceof DOMException && err.name === 'AbortError') return

  const detail = err instanceof DOMException ? `${err.name}: ${err.message}` : String(err)
  rendererLog.error('[VAD] Setup failed', { detail })
}

export function computeAmplitude(audio: Float32Array): number {
  let sum = 0
  for (let i = 0; i < audio.length; i++) sum += (audio[i] ?? 0) ** 2
  return Math.sqrt(sum / (audio.length || 1))
}

export function createVadMessageHandler(isPlaying: () => boolean = () => false): (event: MessageEvent) => void {
  let lastVadEmitMs = 0
  return (event: MessageEvent) => {
    const { type, audio } = event.data as { type?: string; audio?: Float32Array }
    if (type !== 'audio' || !audio) {
      if (type) rendererLog.debug(`[VAD] Event: ${type}`)
      return
    }
    // Use the shared getElectronApi() helper; a lone exception here would defeat the single-bridge invariant.
    const send = getElectronApi()?.runtime?.sendVadAudio
    if (!send) return

    const now = Date.now()
    if (now - lastVadEmitMs < 10) return
    lastVadEmitMs = now
    send({ samples: audio, amplitude: Math.min(1, computeAmplitude(audio) * 2), playing: isPlaying() })
  }
}
