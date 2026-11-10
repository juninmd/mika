import { rendererLog } from '../lib/renderer-logger'
import type { SpeechTranscriptHandler } from './useSpeechRecognition.media'

type SpeechRecognitionEvent = Event & {
  results: SpeechRecognitionResultList
  resultIndex: number
}
type SpeechRecognitionErrorEvent = Event & { error: string }

export interface SpeechRecognitionInstance extends EventTarget {
  lang: string
  continuous: boolean
  interimResults: boolean
  start(): void
  stop(): void
  abort(): void
  onresult: ((e: SpeechRecognitionEvent) => void) | null
  onerror: ((e: SpeechRecognitionErrorEvent) => void) | null
  onend: (() => void) | null
  onstart: (() => void) | null
}

type SpeechRecognitionConstructor = new () => SpeechRecognitionInstance

declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionConstructor
    webkitSpeechRecognition?: SpeechRecognitionConstructor
  }
}

export type CaptureOptions = {
  lang: string
  deviceId?: string
  vadThreshold?: number
  vadSilenceMs?: number
  vadEndpointingEnabled?: boolean
  asrProvider?: string
}

/** `getUserMedia` names for a microphone that is momentarily busy or whose capture was interrupted. */
const TRANSIENT_MIC_OPEN_ERRORS = new Set(['NotReadableError', 'AbortError', 'TrackStartError'])

/** Browser STT must not be attempted offline, nor in Electron without the Google speech service. */
export function isBrowserSttBlocked(prefersBrowserRecognition: boolean, offlineMode: boolean): boolean {
  const inElectronRuntime = typeof navigator !== 'undefined' && /\bElectron\//.test(navigator.userAgent)
  return prefersBrowserRecognition && (offlineMode || inElectronRuntime)
}

export function getSpeechRecognitionClass(): SpeechRecognitionConstructor | undefined {
  if (typeof window === 'undefined') return undefined
  return window.SpeechRecognition ?? window.webkitSpeechRecognition
}

/** Permission/policy refusals: collapsed to the `not-allowed` code the shell treats as fatal (mic off + toast). */
const DENIED_MIC_OPEN_ERRORS = new Set(['NotAllowedError', 'SecurityError'])
/** Missing/unsatisfiable device: the name itself is a fatal token (`isFatalSpeechError`), the free-text message is not. */
const MISSING_MIC_OPEN_ERRORS = new Set(['NotFoundError', 'OverconstrainedError'])

// `getUserMedia` rejects with a DOMException whose human `message` ("Permission denied") would never match the
// fatal tokens, so classify by `name`. NotReadableError/AbortError/TrackStartError are transient busy/interrupted
// errors, tagged microphone-transient-<Name> (not matched by isFatalSpeechError) so they don't disable the mic for good.
export function describeMicOpenFailure(error: unknown): string {
  const { name, message } = (error ?? {}) as { name?: unknown; message?: unknown }
  if (typeof name !== 'string') return 'media-recorder-unavailable'
  if (DENIED_MIC_OPEN_ERRORS.has(name)) return 'not-allowed'
  if (MISSING_MIC_OPEN_ERRORS.has(name)) return name
  if (TRANSIENT_MIC_OPEN_ERRORS.has(name)) return `microphone-transient-${name}`
  return (typeof message === 'string' && message) || name
}

type BrowserRecognitionRefs = {
  activeRef: { current: boolean }
  processedRef: { current: Set<number> }
  onTranscriptRef: { current: SpeechTranscriptHandler }
  onErrorRef: { current: ((error: string) => void) | undefined }
  setListening: (listening: boolean) => void
}

export function wireBrowserRecognition(
  rec: SpeechRecognitionInstance,
  { activeRef, processedRef, onTranscriptRef, onErrorRef, setListening }: BrowserRecognitionRefs,
): void {
  let silenceTimer: NodeJS.Timeout | null = null
  let lastPartialTranscript = ''

  const processFinalResult = (_r: SpeechRecognitionResult, i: number, text: string) => {
    if (processedRef.current.has(i)) return
    processedRef.current.add(i)
    if (!text) return
    if (silenceTimer) clearTimeout(silenceTimer)
    lastPartialTranscript = ''
    onTranscriptRef.current(text, {})
  }

  const scheduleSilenceTimer = () => {
    if (silenceTimer) clearTimeout(silenceTimer)
    silenceTimer = setTimeout(() => {
      if (!activeRef.current) return
      if (!lastPartialTranscript.trim()) return
      rendererLog.debug('[STT] Silence detected, sending partial transcript:', lastPartialTranscript)
      onTranscriptRef.current(lastPartialTranscript.trim(), {})
      lastPartialTranscript = ''
      try {
        rec.abort()
      } catch {}
    }, 900)
  }

  rec.onresult = (e: SpeechRecognitionEvent) => {
    let interimTranscript = ''

    for (let i = e.resultIndex; i < e.results.length; i++) {
      const r = e.results[i]
      const text = r[0].transcript.trim()

      if (r.isFinal) {
        processFinalResult(r, i, text)
        continue
      }

      interimTranscript += r[0].transcript
    }

    if (interimTranscript && interimTranscript !== lastPartialTranscript) {
      lastPartialTranscript = interimTranscript
      scheduleSilenceTimer()
    }
  }

  rec.onerror = (e: SpeechRecognitionErrorEvent) => {
    if (silenceTimer) clearTimeout(silenceTimer)
    if (e.error !== 'aborted' && e.error !== 'no-speech') {
      onErrorRef.current?.(e.error)
      rendererLog.warn('[STT] Error:', e.error)
    }
  }

  rec.onstart = () => {
    rendererLog.info('[STT] Listening...')
  }

  rec.onend = () => {
    if (!activeRef.current && silenceTimer) clearTimeout(silenceTimer)
    if (activeRef.current) {
      try {
        rec.start()
      } catch {
        activeRef.current = false
        setListening(false)
      }
    } else {
      setListening(false)
    }
  }
}
