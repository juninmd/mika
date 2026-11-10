import { getLocale } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useRef, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { createSpeechSessionController, type SpeechSessionController } from '../lib/speech-session'
import {
  type CaptureOptions,
  describeMicOpenFailure,
  getSpeechRecognitionClass,
  isBrowserSttBlocked,
  type SpeechRecognitionInstance,
  wireBrowserRecognition,
} from './speech-recognition-browser'
import { type SpeechTranscriptHandler, startMediaRecorderTranscription } from './useSpeechRecognition.media'

interface UseSpeechRecognitionOptions {
  lang?: string
  asrProvider?: string
  deviceId?: string

  vadThreshold?: number
  vadSilenceMs?: number
  vadEndpointingEnabled?: boolean

  offlineMode?: boolean
  onTranscript: SpeechTranscriptHandler
  onError?: (error: string) => void
}

export function useSpeechRecognition({
  lang = getLocale(),
  // Matches `voice.asrProvider`'s schema default (config/schemas/voice.rs) — see the comment on
  // `useComposerSpeech`'s own fallback for why this only affects the milestone log, not which
  // capture path runs.
  asrProvider = 'nemotron',
  deviceId,
  vadThreshold,
  vadSilenceMs,
  vadEndpointingEnabled,
  offlineMode = false,
  onTranscript,
  onError,
}: UseSpeechRecognitionOptions) {
  const [listening, setListening] = useState(false)
  const [processing, setProcessing] = useState(false)
  const recognitionRef = useRef<SpeechRecognitionInstance | null>(null)

  const activeRef = useRef(false)
  const processedRef = useRef<Set<number>>(new Set())
  const onTranscriptRef = useRef(onTranscript)
  const onErrorRef = useRef(onError)
  onTranscriptRef.current = onTranscript
  onErrorRef.current = onError
  // Read at start-time, not closed over, so start/stop stay stable identities and device/lang/vad
  // changes don't re-trigger the shell effect (which caused rapid mic open/close bursts).
  const optionsRef = useRef({
    asrProvider,
    lang,
    deviceId,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
    offlineMode,
  })
  optionsRef.current = { asrProvider, lang, deviceId, vadThreshold, vadSilenceMs, vadEndpointingEnabled, offlineMode }
  // One controller per hook, created once: guards a single mic session even against stop→start mid-getUserMedia.
  const fallbackRef = useRef<SpeechSessionController<CaptureOptions> | null>(null)
  if (!fallbackRef.current) {
    fallbackRef.current = createSpeechSessionController(
      (opts: CaptureOptions) =>
        startMediaRecorderTranscription({
          ...opts,
          onTranscript: (text, meta) => onTranscriptRef.current(text, meta),
          onError: (error) => onErrorRef.current?.(error),
          onProcessingChange: (next) => setProcessing(next),
        }),
      (error) => {
        activeRef.current = false
        setListening(false)
        setProcessing(false)
        onErrorRef.current?.(describeMicOpenFailure(error))
      },
    )
  }

  const SpeechRecognitionClass = getSpeechRecognitionClass()

  const prefersBrowserRecognition = asrProvider === 'browser'
  const canUseMediaRecorderFallback =
    typeof navigator !== 'undefined' &&
    typeof MediaRecorder !== 'undefined' &&
    Boolean(navigator.mediaDevices?.getUserMedia)

  const browserRecognitionBlocked = isBrowserSttBlocked(prefersBrowserRecognition, offlineMode)

  const supported = prefersBrowserRecognition
    ? Boolean(SpeechRecognitionClass) && !browserRecognitionBlocked
    : canUseMediaRecorderFallback

  const stopFallback = useCallback(() => {
    fallbackRef.current?.stop()
  }, [])

  const setPaused = useCallback((paused: boolean) => {
    fallbackRef.current?.setPaused(paused)
  }, [])

  // Stable identity on purpose (deps []); options are read from the ref at call time instead.
  const start = useCallback(() => {
    if (activeRef.current) return

    const {
      asrProvider: provider,
      offlineMode: offline,
      lang: language,
      deviceId: captureDeviceId,
      vadThreshold: captureVadThreshold,
      vadSilenceMs: captureVadSilenceMs,
      vadEndpointingEnabled: captureVadEndpointingEnabled,
    } = optionsRef.current
    const prefersBrowser = provider === 'browser'

    if (isBrowserSttBlocked(prefersBrowser, offline)) {
      onErrorRef.current?.(offline ? 'offline-mode-blocks-browser-stt' : 'browser-stt-unavailable-in-electron')
      return
    }

    const RecognitionClass = getSpeechRecognitionClass()

    activeRef.current = true
    processedRef.current = new Set()
    setListening(true)

    if (!prefersBrowser) {
      // Controller refuses a start while one is live/opening; track actual result or activeRef locks out further starts.
      const opened =
        fallbackRef.current?.start({
          lang: language,
          deviceId: captureDeviceId,
          vadThreshold: captureVadThreshold,
          vadSilenceMs: captureVadSilenceMs,
          vadEndpointingEnabled: captureVadEndpointingEnabled,
          asrProvider: provider,
        }) === true
      activeRef.current = opened
      setListening(opened)
      return
    }

    if (!RecognitionClass) {
      activeRef.current = false
      setListening(false)
      onErrorRef.current?.('browser-stt-unavailable')
      return
    }

    const rec = new RecognitionClass()
    rec.lang = language
    rec.continuous = true
    rec.interimResults = true
    recognitionRef.current = rec

    wireBrowserRecognition(rec, { activeRef, processedRef, onTranscriptRef, onErrorRef, setListening })

    try {
      rec.start()
    } catch (err) {
      activeRef.current = false
      setListening(false)
      const msg = toDisplayError(err, String(err))
      onErrorRef.current?.(msg)
      rendererLog.warn('[STT] Failed to start:', err)
    }
  }, [])

  const stop = useCallback(() => {
    activeRef.current = false
    stopFallback()
    try {
      recognitionRef.current?.stop()
    } catch {
      // Stopping an already-ended recognition throws harmlessly.
    }
    recognitionRef.current = null
    setListening(false)
    setProcessing(false)
  }, [stopFallback])

  useEffect(() => {
    return () => {
      activeRef.current = false
      stopFallback()
      try {
        recognitionRef.current?.abort()
      } catch {}
    }
  }, [stopFallback])

  return { listening, processing, supported, start, stop, setPaused }
}
