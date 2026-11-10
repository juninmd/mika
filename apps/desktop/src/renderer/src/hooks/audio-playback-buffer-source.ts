// Plays TTS audio via AudioBufferSourceNode (low latency). No React import.

import type { LipSyncSource, WordBoundary } from '../lib/lip-sync-source'
import { rendererLog } from '../lib/renderer-logger'
import { decodeBase64Audio } from './audio-playback-codec'
import type { PlaybackRefs } from './audio-playback-types'

const AUDIO_STOP_FADE_SEC = 0.005
const AUDIO_START_FADE_SEC = 0.005

// Returns false if decodeAudioData fails, to fall back to <audio>.
export async function tryPlayBufferSource(
  ctx: AudioContext,
  lipSync: LipSyncSource,
  normalizedBase64: string,
  preDecoded: Promise<AudioBuffer | null> | undefined,
  words: WordBoundary[] | undefined,
  targetVolume: number,
  refs: PlaybackRefs,
): Promise<boolean> {
  let audioBuffer: AudioBuffer | null
  try {
    // Queued chunk was already decoded while the previous one played — no gap between phrases.
    audioBuffer = preDecoded ? await preDecoded : await decodeBase64Audio(ctx, normalizedBase64)
    if (!audioBuffer) throw new Error('pre-decode failed')
  } catch (decodeErr) {
    // Cancelled while decoding: nothing left to play, and the <audio> fallback would speak a dead phrase.
    if (refs.playbackTokenRef.current !== refs.token) return true
    const msg = decodeErr instanceof Error ? `${decodeErr.name}: ${decodeErr.message}` : String(decodeErr)
    rendererLog.warn('[AudioPlayback] decodeAudioData failed, falling back to <audio> element:', msg)
    return false
  }
  if (refs.playbackTokenRef.current !== refs.token) return true

  rendererLog.info('[AudioPlayback] Playing via AudioContext', {
    duration: audioBuffer.duration,
    sampleRate: audioBuffer.sampleRate,
    channels: audioBuffer.numberOfChannels,
  })

  const source = ctx.createBufferSource()
  source.buffer = audioBuffer
  const gainNode = lipSync.attachBufferSource(source)
  const startNow = ctx.currentTime
  gainNode.gain.setValueAtTime(0.001, startNow)
  gainNode.gain.linearRampToValueAtTime(targetVolume, startNow + AUDIO_START_FADE_SEC)

  refs.stopPlaybackRef.current = () => {
    source.onended = null
    const fadeEnd = ctx.currentTime + AUDIO_STOP_FADE_SEC
    try {
      gainNode.gain.cancelScheduledValues(ctx.currentTime)
      gainNode.gain.setValueAtTime(gainNode.gain.value, ctx.currentTime)
      gainNode.gain.linearRampToValueAtTime(0, fadeEnd)
    } catch (e) {
      rendererLog.warn('[AudioPlayback] Failed to fade out gain:', e)
    }
    try {
      source.stop(fadeEnd)
    } catch (e) {
      rendererLog.warn('[AudioPlayback] Failed to stop source:', e)
    }

    setTimeout(
      () => {
        try {
          source.disconnect()
          gainNode.disconnect()
        } catch (e) {
          rendererLog.warn('[AudioPlayback] Failed to disconnect source:', e)
        }
      },
      AUDIO_STOP_FADE_SEC * 1000 + 20,
    )
  }

  refs.setIsPlaying(true)
  lipSync.setPlaying(true)
  source.start(0)

  if (words && words.length > 0) lipSync.setWordBoundaries(words)

  await new Promise<void>((resolve) => {
    refs.playbackResolveRef.current = () => {
      refs.playbackResolveRef.current = null
      resolve()
    }
    source.onended = () => {
      refs.playbackResolveRef.current = null
      refs.finishPlayback()
      resolve()
    }
  })
  return true
}
