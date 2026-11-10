import { float32ArrayToInt16 } from '@mika/sdk/utils/audio'
import { clamp01 } from '@mika/sdk/utils/clamp'
import { toDisplayError } from '@mika/sdk-ui'
import { rendererLog } from '../lib/renderer-logger'
import type { getElectronApi } from '../lib/shell-api'
import { prependPreRoll, preRollDurationMs } from './speech-preroll'

type TranscribeResponse = {
  success: boolean
  data?: { text: string }
  error?: string
}

export type MediaRecorderSession = {
  stop: () => void

  setPaused: (paused: boolean) => void
}

export type MediaRecorderFallbackOptions = {
  lang: string
  deviceId?: string

  vadThreshold?: number
  vadSilenceMs?: number
  vadEndpointingEnabled?: boolean
  // Not used to pick the engine (the backend resolves that from its own config) — only to label
  // the durable milestone log with which one config asked for at capture time.
  asrProvider?: string
  onTranscript: SpeechTranscriptHandler
  onError?: (error: string) => void
  onProcessingChange?: (processing: boolean) => void
}

export type MediaRecorderContext = {
  closed: boolean
  lang: string
  asrProvider?: string
  onTranscript: SpeechTranscriptHandler
  onError?: (error: string) => void
  onProcessingChange?: (processing: boolean) => void
  runtime: ReturnType<typeof getElectronApi>['runtime']
}

/** Durable milestone, no transcript text — the bridge's `runtime.log`, the existing path to
 * the on-disk log (`modules/system/log_files.rs`), same one the settings Logs panel reads back. */
export function logSttMilestone(
  runtime: MediaRecorderContext['runtime'],
  msg: string,
  meta?: Record<string, unknown>,
): void {
  void runtime.log({ level: 'debug', scope: 'stt', msg, meta })
}

// durationMs is the captured AUDIO window, not the model's processing time.
export type SpeechTranscriptHandler = (text: string, meta: { durationMs?: number }) => void

const SPEECH_START_LEVEL = 0.12
const SPEECH_STOP_LEVEL = 0.07
// Default only — the live listening path reads `voice.vadSilenceMs` instead (same value the
// "Duração de Silêncio para Finalizar" setting saves) once shell config sync has loaded it; this is
// what a session uses before that first load and what config-less tests exercise. Matches the
// schema default in config/schemas/voice.rs.
export const SILENCE_HANGOVER_MS = process.env.NODE_ENV === 'test' ? 10 : 1200
const MIN_UTTERANCE_MS = process.env.NODE_ENV === 'test' ? 10 : 350
export const MAX_UTTERANCE_MS = 45_000
export const VAD_POLL_MS = process.env.NODE_ENV === 'test' ? 5 : 100

export const NOISE_FLOOR_RISE_PER_TICK = 0.002

export const MAX_NOISE_FLOOR = 0.4
export const NOISE_MARGIN = 0.1

const IGNORABLE_TRANSCRIBE_ERROR =
  /(no speech|silence|silent|empty|sem fala|sem áudio|audio vazio|fila de transcrição|queue)/i

function isIgnorableTranscribeError(error: string): boolean {
  return IGNORABLE_TRANSCRIBE_ERROR.test(error)
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''

  for (const byte of bytes) binary += String.fromCharCode(byte)

  return btoa(binary)
}

const PCM_SAMPLE_RATE = 16_000

export const PCM_MIME_TYPE = `audio/pcm;rate=${PCM_SAMPLE_RATE}`

export async function blobToPcm16(blob: Blob): Promise<Uint8Array> {
  const encoded = await blob.arrayBuffer()

  const decodeCtx = new AudioContext()
  let decoded: AudioBuffer
  try {
    decoded = await decodeCtx.decodeAudioData(encoded)
  } finally {
    void decodeCtx.close()
  }

  const frames = Math.max(1, Math.ceil(decoded.duration * PCM_SAMPLE_RATE))
  const offline = new OfflineAudioContext(1, frames, PCM_SAMPLE_RATE)
  const source = offline.createBufferSource()
  source.buffer = decoded
  source.connect(offline.destination)
  source.start()
  const resampled = await offline.startRendering()

  const samples = resampled.getChannelData(0)
  const pcm = float32ArrayToInt16(samples)
  return new Uint8Array(pcm.buffer)
}

export function dropReason(closed: boolean, confirmed: boolean, durationMs: number, chunkCount: number): string | null {
  if (closed) return 'closed'
  if (!confirmed) return 'below-speech-gate'
  if (durationMs < MIN_UTTERANCE_MS) return 'too-short'
  if (chunkCount === 0) return 'no-audio-chunks'
  return null
}

/** `voice.vadEndpointingEnabled` gates silence-based auto-finish entirely — same semantics as the
 * Rust barge-in detector (`interruption_detector.rs`): disabled means only the hard ceiling ends
 * the utterance. `voice.vadSilenceMs` is the live hangover once enabled. */
export function shouldFinishUtterance(
  silentForMs: number,
  elapsedMs: number,
  hangoverMs: number,
  endpointingEnabled: boolean,
): boolean {
  if (elapsedMs >= MAX_UTTERANCE_MS) return true
  return endpointingEnabled && silentForMs >= hangoverMs
}

export async function transcribeUtterance(
  blob: Blob,
  context: MediaRecorderContext,
  durationMs: number,
  preRoll?: Float32Array,
): Promise<void> {
  if (context.closed || blob.size === 0) return

  context.onProcessingChange?.(true)
  try {
    const pcm = await blobToPcm16(blob)
    const combined = prependPreRoll(pcm, preRoll)
    // The window sent for transcription now includes the pre-roll — the syllable that tripped the
    // gate ships with it, so the reported window covers what was actually transcribed.
    const totalDurationMs = durationMs + preRollDurationMs(preRoll)
    const startedAt = performance.now()
    const result = (await context.runtime.transcribeAudio({
      audioBase64: bytesToBase64(combined),
      mimeType: PCM_MIME_TYPE,
      language: context.lang,
    })) as TranscribeResponse
    const latencyMs = Math.round(performance.now() - startedAt)
    logSttMilestone(context.runtime, 'transcript-latency', { latencyMs, provider: context.asrProvider })

    if (result.success) {
      const transcript = result.data?.text?.trim()

      rendererLog.debug('[STT] transcrição concluida', { chars: transcript?.length ?? 0, durationMs: totalDurationMs })
      // Audio window travels with the transcript so later checks can ask if the text plausibly came from that sound.
      if (transcript) context.onTranscript(transcript, { durationMs: totalDurationMs })
      return
    }

    const error = result.error ?? 'transcription-failed'
    rendererLog.warn('[STT] transcrição falhou', { error })
    if (!isIgnorableTranscribeError(error)) {
      context.onError?.(error)
    }
  } catch (error) {
    if (!context.closed) {
      context.onError?.(toDisplayError(error, 'transcription-failed'))
    }
  } finally {
    context.onProcessingChange?.(false)
  }
}

export function scaleVadGates(threshold: number | undefined): { start: number; stop: number } {
  const t = clamp01(threshold ?? Number.NaN, 0.5)
  const factor = t / 0.5
  return {
    start: Math.max(0.03, SPEECH_START_LEVEL * factor),
    stop: Math.max(0.02, SPEECH_STOP_LEVEL * factor),
  }
}
