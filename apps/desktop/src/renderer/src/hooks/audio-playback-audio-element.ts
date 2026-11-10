// Fallback TTS playback via HTMLAudioElement (when decodeAudioData fails). No React import.
import { toErrorMessage } from '@mika/sdk/utils/object'
import type { LipSyncSource, WordBoundary } from '../lib/lip-sync-source'
import { rendererLog } from '../lib/renderer-logger'
import { applyOutputDevice } from './audio-playback-device'
import type { PlaybackRefs } from './audio-playback-types'

export async function playViaAudioElement(
  mimeType: string,
  normalizedBase64: string,
  volume: number,
  lipSync: LipSyncSource,
  outputSinkId: string,
  words: WordBoundary[] | undefined,
  refs: PlaybackRefs,
): Promise<void> {
  if (refs.playbackTokenRef.current !== refs.token) return
  const audioEl = new Audio()
  audioEl.src = `data:${mimeType};base64,${normalizedBase64}`
  audioEl.volume = volume

  let elementRouted = false
  try {
    lipSync.attachMediaElement(audioEl)
    elementRouted = true
  } catch (e) {
    rendererLog.warn('[AudioPlayback] createMediaElementSource failed, playing direct:', e)
  }
  if (!elementRouted) await applyOutputDevice(audioEl, outputSinkId)
  // Cancelled while the sink was being applied: don't claim the playback refs of the newer one.
  if (refs.playbackTokenRef.current !== refs.token) {
    audioEl.removeAttribute('src')
    return
  }

  refs.stopPlaybackRef.current = () => {
    audioEl.onended = null
    audioEl.onerror = null
    try {
      audioEl.pause()
    } catch (e) {
      rendererLog.warn('[AudioPlayback] Failed to pause audio element:', e)
    }
    audioEl.removeAttribute('src')
    audioEl.load()
  }

  refs.setIsPlaying(true)
  lipSync.setPlaying(true)

  await new Promise<void>((resolve) => {
    refs.playbackResolveRef.current = () => {
      refs.playbackResolveRef.current = null
      resolve()
    }
    const cleanup = () => {
      refs.playbackResolveRef.current = null
      refs.finishPlayback()
      resolve()
    }
    audioEl.onended = cleanup
    audioEl.onerror = (e) => {
      if (refs.playbackTokenRef.current === refs.token) {
        const err = audioEl.error
        rendererLog.warn(`[AudioPlayback] HTMLAudio error: code=${err?.code} message=${err?.message ?? String(e)}`)
      }
      cleanup()
    }
    audioEl
      .play()
      .then(() => {
        if (refs.playbackTokenRef.current === refs.token && words && words.length > 0) {
          lipSync.setWordBoundaries(words)
        }
      })
      .catch((playErr) => {
        if (refs.playbackTokenRef.current !== refs.token) {
          resolve()
          return
        }
        const name = playErr instanceof DOMException ? playErr.name : 'UnknownError'
        const msg = toErrorMessage(playErr)
        rendererLog.warn(`[AudioPlayback] HTMLAudio play() rejected (${name}): ${msg}`)
        if (name === 'NotAllowedError') {
          rendererLog.error('[AudioPlayback] Autoplay blocked — enable autoplay in Electron BrowserWindow.')
        }
        cleanup()
      })
  })
}
