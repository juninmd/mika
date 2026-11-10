import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useRef, useState } from 'react'
import { LipSyncSource, type WordBoundary } from '../lib/lip-sync-source'
import { SILENT_VOWELS, type VowelWeights } from '../lib/lip-sync-vowels'
import { rendererLog } from '../lib/renderer-logger'
import { admitToTtsQueue } from '../lib/tts-queue'
import { decodeBase64Audio, normalizeBase64 } from './audio-playback-codec'
import { applyOutputDevice, getConfiguredOutputDeviceId } from './audio-playback-device'
import { runPlayback } from './audio-playback-engine'
import type { PlaybackRefs, QueuedAudio } from './audio-playback-types'
import { useBargeInVad } from './useBargeInVad'

const LIP_SYNC_LEAD_SEC = 0.05

export function useAudioPlayback(
  volumeRef: React.MutableRefObject<number>,
  vadEnabled = false,
  ttsVolumeRef?: React.MutableRefObject<number>,
  inputDeviceId?: string,
) {
  const [isPlaying, setIsPlaying] = useState(false)
  const ctxRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const playbackTokenRef = useRef(0)
  const stopPlaybackRef = useRef<(() => void) | null>(null)

  const playbackResolveRef = useRef<(() => void) | null>(null)

  const lipSyncRef = useRef<LipSyncSource | null>(null)

  const vowelsRef = useRef<VowelWeights>({ ...SILENT_VOWELS })

  const audioQueueRef = useRef<QueuedAudio[]>([])
  const isProcessingQueueRef = useRef(false)

  const getCtx = useCallback((): { ctx: AudioContext; analyser: AnalyserNode } => {
    if (!ctxRef.current || ctxRef.current.state === 'closed') {
      ctxRef.current = new AudioContext()
      analyserRef.current = ctxRef.current.createAnalyser()
      analyserRef.current.fftSize = 1024
      analyserRef.current.smoothingTimeConstant = 0.55
      // The avatar draws the mouth one frame after reading the analyser; delaying audio syncs the syllable to it.
      const lipSyncLead = ctxRef.current.createDelay(0.2)
      lipSyncLead.delayTime.value = LIP_SYNC_LEAD_SEC
      analyserRef.current.connect(lipSyncLead).connect(ctxRef.current.destination)
    }
    if (!ctxRef.current || !analyserRef.current) {
      throw new Error('Audio context initialization failed')
    }
    return { ctx: ctxRef.current, analyser: analyserRef.current }
  }, [])

  const getLipSync = useCallback((): LipSyncSource => {
    if (!lipSyncRef.current) {
      const { ctx, analyser } = getCtx()
      lipSyncRef.current = new LipSyncSource({ ctx, analyser, volumeRef, vowelsRef })
    }
    return lipSyncRef.current
  }, [getCtx, volumeRef])

  useEffect(() => {
    const { ctx } = getCtx()
    if (ctx.state === 'suspended') {
      ctx.resume().catch((e) => rendererLog.warn('[AudioPlayback] Failed to resume context:', e))
    }

    return () => {
      lipSyncRef.current?.dispose()
      lipSyncRef.current = null
      stopPlaybackRef.current?.()
      ctxRef.current?.close().catch((e) => rendererLog.warn('[AudioPlayback] Failed to close context:', e))
    }
  }, [getCtx])

  const isPlayingRef = useRef(isPlaying)
  isPlayingRef.current = isPlaying
  // The queue flag spans the gaps between chunks, where isPlaying briefly drops to false.
  const readIsPlaying = useCallback(() => isProcessingQueueRef.current || isPlayingRef.current, [])
  useBargeInVad(getCtx, vadEnabled, inputDeviceId, readIsPlaying)

  const resetPlaybackState = useCallback(() => {
    setIsPlaying(false)

    lipSyncRef.current?.setPlaying(false)
  }, [])

  const cancelCurrentPlayback = useCallback(() => {
    playbackTokenRef.current += 1

    playbackResolveRef.current?.()
    playbackResolveRef.current = null
    stopPlaybackRef.current?.()
    stopPlaybackRef.current = null
    resetPlaybackState()
  }, [resetPlaybackState])

  // biome-ignore lint/correctness/useExhaustiveDependencies: ttsVolumeRef is a stable ref read at call time (ttsVolumeRef.current) — it is never stale, so adding it as a dep would only force needless callback identity churn.
  const play = useCallback(
    async (
      audioBase64: string,
      mimeType = 'audio/mpeg',
      words?: WordBoundary[],
      preDecoded?: Promise<AudioBuffer | null>,
    ): Promise<void> => {
      const { ctx } = getCtx()
      const lipSync = getLipSync()
      cancelCurrentPlayback()
      const token = playbackTokenRef.current
      const outputSinkId = getConfiguredOutputDeviceId()
      await applyOutputDevice(ctx, outputSinkId)

      const finishPlayback = () => {
        if (playbackTokenRef.current !== token) return
        stopPlaybackRef.current = null
        resetPlaybackState()
      }

      try {
        const playbackRefs: PlaybackRefs = {
          token,
          playbackTokenRef,
          playbackResolveRef,
          stopPlaybackRef,
          setIsPlaying,
          finishPlayback,
        }
        await runPlayback(
          ctx,
          lipSync,
          audioBase64,
          mimeType,
          words,
          preDecoded,
          outputSinkId,
          ttsVolumeRef,
          resetPlaybackState,
          playbackRefs,
        )
      } catch (err) {
        if (playbackTokenRef.current !== token) return
        const name = err instanceof DOMException ? err.name : err instanceof Error ? err.constructor.name : 'Error'
        const msg = toErrorMessage(err)
        rendererLog.error(`[AudioPlayback] Failed to play TTS audio (${name}): ${msg}`)
        resetPlaybackState()
      }
    },
    [cancelCurrentPlayback, getCtx, getLipSync, resetPlaybackState],
  )

  const processQueue = useCallback(async () => {
    if (isProcessingQueueRef.current) return
    isProcessingQueueRef.current = true
    try {
      while (audioQueueRef.current.length > 0) {
        const item = audioQueueRef.current.shift()
        if (item) {
          try {
            await play(item.audio, item.mimeType, item.words, item.decoded)
          } catch (playbackErr) {
            rendererLog.warn('[AudioPlayback] Queued item playback failed, continuing queue:', playbackErr)
            resetPlaybackState()
          }
        }
      }
    } finally {
      isProcessingQueueRef.current = false
    }
  }, [play, resetPlaybackState])

  const enqueue = useCallback(
    (audio: string, mimeType = 'audio/mpeg', words?: WordBoundary[]) => {
      const item: QueuedAudio = { audio, mimeType, words }
      if (admitToTtsQueue(audioQueueRef.current, item) === 'dropped') {
        rendererLog.warn('[AudioPlayback] TTS queue full, dropping newest chunk', {
          pending: audioQueueRef.current.length,
        })
        return
      }
      // Only decode accepted items — the cap exists to contain a looping producer.
      item.decoded = decodeBase64Audio(getCtx().ctx, normalizeBase64(audio)).catch(() => null)
      processQueue()
    },
    [processQueue, getCtx],
  )

  const stopAll = useCallback(() => {
    audioQueueRef.current = []
    cancelCurrentPlayback()
  }, [cancelCurrentPlayback])

  return { play, enqueue, stopAll, isPlaying, vowelsRef }
}
