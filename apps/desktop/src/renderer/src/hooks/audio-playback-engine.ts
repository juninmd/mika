// Orchestrates one playback attempt: buffer-source first, <audio> element fallback. No React import.
import { clamp01 } from '@mika/sdk/utils/clamp'
import type { LipSyncSource, WordBoundary } from '../lib/lip-sync-source'
import { rendererLog } from '../lib/renderer-logger'
import { playViaAudioElement } from './audio-playback-audio-element'
import { tryPlayBufferSource } from './audio-playback-buffer-source'
import { isLikelyBase64, normalizeBase64 } from './audio-playback-codec'
import type { PlaybackRefs, Ref } from './audio-playback-types'

export async function resumeContextIfSuspended(ctx: AudioContext): Promise<void> {
  if (ctx.state === 'suspended') {
    rendererLog.info('[AudioPlayback] Resuming suspended AudioContext')
    await ctx.resume()
  }
  if (ctx.state !== 'running') {
    rendererLog.warn('[AudioPlayback] AudioContext not running after resume, state:', ctx.state)
  }
}

export async function runPlayback(
  ctx: AudioContext,
  lipSync: LipSyncSource,
  audioBase64: string,
  mimeType: string,
  words: WordBoundary[] | undefined,
  preDecoded: Promise<AudioBuffer | null> | undefined,
  outputSinkId: string,
  ttsVolumeRef: Ref<number> | undefined,
  resetPlaybackState: () => void,
  refs: PlaybackRefs,
): Promise<void> {
  await resumeContextIfSuspended(ctx)

  const normalizedBase64 = normalizeBase64(audioBase64)

  if (!isLikelyBase64(normalizedBase64)) {
    rendererLog.warn('[AudioPlayback] Ignoring invalid base64 TTS payload', {
      length: normalizedBase64.length,
      preview: normalizedBase64.slice(0, 40),
    })
    resetPlaybackState()
    return
  }

  const targetVolume = clamp01(ttsVolumeRef?.current ?? 1, 1)

  const playedViaBufferSource = await tryPlayBufferSource(
    ctx,
    lipSync,
    normalizedBase64,
    preDecoded,
    words,
    targetVolume,
    refs,
  )
  if (playedViaBufferSource) return

  await playViaAudioElement(mimeType, normalizedBase64, targetVolume, lipSync, outputSinkId, words, refs)
}
