import { createMicLevelMeter, getUserMediaWithFallback } from '../lib/mic-analyser'
import { publishMicLevel, resetMicLevel } from '../lib/mic-level-bus'
import { stopStream } from '../lib/mic-stream'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { logVadSetupFailure, VAD_WORKLET_PATH } from './audio-playback-vad'
import { createPreRollBuffer, type PreRollBuffer } from './speech-preroll'
import {
  dropReason,
  logSttMilestone,
  MAX_NOISE_FLOOR,
  type MediaRecorderContext,
  type MediaRecorderFallbackOptions,
  type MediaRecorderSession,
  NOISE_FLOOR_RISE_PER_TICK,
  NOISE_MARGIN,
  SILENCE_HANGOVER_MS,
  scaleVadGates,
  shouldFinishUtterance,
  transcribeUtterance,
  VAD_POLL_MS,
} from './speech-transcribe'

export type { SpeechTranscriptHandler } from './speech-transcribe'

type MicMeter = ReturnType<typeof createMicLevelMeter>

/** Continuous 16kHz mono tap, same worklet the barge-in path already loads — feeds the pre-roll
 * ring buffer so onset clipping (the gate-opening syllable) doesn't lose audio while nothing is
 * "recording" yet. Best-effort: a webview without AudioWorklet support just runs without pre-roll,
 * same as today. */
async function setupPreRollTap(mediaStream: MediaStream, preRoll: PreRollBuffer): Promise<() => void> {
  const ctx = new AudioContext()
  try {
    await ctx.audioWorklet.addModule(VAD_WORKLET_PATH())
  } catch (error) {
    // No worklet support / module 404: nothing will ever use this context, so don't leave it running.
    void ctx.close().catch(() => {})
    throw error
  }
  const node = new AudioWorkletNode(ctx, 'vad-processor', {
    numberOfInputs: 1,
    numberOfOutputs: 1,
    processorOptions: { sampleRate: ctx.sampleRate },
  })
  node.port.onmessage = (event: MessageEvent) => {
    const { type, audio } = event.data as { type?: string; audio?: Float32Array }
    if (type === 'audio' && audio) preRoll.push(audio)
  }
  ctx.createMediaStreamSource(mediaStream).connect(node)
  return () => {
    node.port.onmessage = null
    node.disconnect()
    void ctx.close()
  }
}

/**
 * Contêineres aceitos, do preferido para o de último recurso.
 *
 * Este é o caminho PRINCIPAL de voz, não um fallback: `asrProvider` nasce como `'electron'`.
 * Sob Electron o motor era sempre o mesmo Chromium e `new MediaRecorder(stream)` bastava; em
 * Tauri cada SO traz um webview diferente — WebView2, WKWebView e WebKitGTK — e o WebKit não
 * grava webm/opus. Sem negociar, a construção lança `NotSupportedError` e a captura de voz
 * morre calada em macOS e Linux.
 */
const RECORDER_MIME_TYPES = [
  'audio/webm;codecs=opus',
  'audio/webm',
  'audio/mp4;codecs=mp4a.40.2',
  'audio/mp4',
  'audio/ogg;codecs=opus',
] as const

/** Sem opção quando nenhum candidato serve: aí o padrão do motor é a melhor aposta que resta. */
function recorderOptions(): MediaRecorderOptions | undefined {
  if (typeof MediaRecorder?.isTypeSupported !== 'function') return undefined

  const supported = RECORDER_MIME_TYPES.find((type) => MediaRecorder.isTypeSupported(type))
  return supported ? { mimeType: supported } : undefined
}

export async function startMediaRecorderTranscription({
  lang,
  deviceId,
  vadThreshold,
  vadSilenceMs,
  vadEndpointingEnabled = true,
  asrProvider,
  onTranscript,
  onError,
  onProcessingChange,
}: MediaRecorderFallbackOptions): Promise<MediaRecorderSession> {
  if (typeof navigator === 'undefined' || !navigator.mediaDevices?.getUserMedia) {
    throw new Error('media-devices-unavailable')
  }

  if (typeof MediaRecorder === 'undefined') {
    throw new Error('media-recorder-unavailable')
  }

  const stream = await getUserMediaWithFallback(deviceId)
  // Past this point nothing holds the stream but this function: a throw before the session object
  // exists must close it, or the mic stays live with no handle left to stop it.
  let setup: { runtime: ReturnType<typeof getElectronApi>['runtime']; context: MediaRecorderContext; meter: MicMeter }
  try {
    const runtime = getElectronApi().runtime
    setup = {
      runtime,
      context: { closed: false, lang, asrProvider, onTranscript, onError, onProcessingChange, runtime },
      meter: createMicLevelMeter(stream),
    }
  } catch (error) {
    stopStream(stream)
    throw error
  }
  const { runtime, context, meter } = setup
  const gates = scaleVadGates(vadThreshold)
  const hangoverMs = vadSilenceMs ?? SILENCE_HANGOVER_MS

  const preRoll = createPreRollBuffer()
  let preRollTapCancelled = false
  let stopPreRollTap: (() => void) | null = null
  setupPreRollTap(stream, preRoll)
    .then((stop) => {
      if (preRollTapCancelled) {
        stop()
        return
      }
      stopPreRollTap = stop
    })
    .catch((error: unknown) => logVadSetupFailure(error, context.closed))

  let recorder: MediaRecorder | null = null
  let utteranceStart = 0
  let lastVoiceAt = 0
  let noiseFloor = 1
  let paused = false

  let verdict = { confirmed: false, peak: 0 }

  const finishUtterance = () => {
    if (!recorder) return
    const active = recorder
    recorder = null
    // The ring buffer keeps rolling through the whole recording, so right after an utterance ends
    // it still holds that utterance's own trailing audio. Reset here so a next onset that opens the
    // gate again before PRE_ROLL_MS of fresh silence accumulates never replays already-sent audio.
    preRoll.reset()
    try {
      active.stop()
    } catch {
      // Stopping an already-stopped MediaRecorder throws harmlessly; silence here is intentional.
    }
  }

  const beginUtterance = () => {
    const chunks: Blob[] = []
    const rec = new MediaRecorder(stream, recorderOptions())
    recorder = rec
    utteranceStart = performance.now()
    lastVoiceAt = utteranceStart
    const startedAt = utteranceStart
    const mine = { confirmed: false, peak: 0 }
    verdict = mine
    // Snapshotted here, not read later: the ring buffer keeps rolling once the recorder takes over,
    // so this closure — not `preRoll` at emit time — is the onset audio for THIS utterance.
    const preRollSnapshot = preRoll.snapshot()
    logSttMilestone(runtime, 'utterance-start')

    rec.ondataavailable = (event) => {
      if (event.data.size > 0) chunks.push(event.data)
    }
    rec.onstop = () => {
      const durationMs = performance.now() - startedAt

      const drop = dropReason(context.closed, mine.confirmed, durationMs, chunks.length)
      logSttMilestone(runtime, 'utterance-stop', {
        durationMs: Math.round(durationMs),
        dropReason: drop,
        provider: asrProvider,
      })
      if (drop) {
        if (drop !== 'closed') {
          rendererLog.debug('[STT] utterance descartado', {
            motivo: drop,
            durationMs: Math.round(durationMs),
            peakLevel: Number(mine.peak.toFixed(3)),
            speechGate: Number(effectiveGates().speech.toFixed(3)),
            noiseFloor: Number(noiseFloor.toFixed(3)),
          })
        }
        return
      }
      const blob = new Blob(chunks, { type: rec.mimeType })
      rendererLog.debug('[STT] enviando utterance', {
        durationMs: Math.round(durationMs),
        bytes: blob.size,
        peakLevel: Number(mine.peak.toFixed(3)),
      })
      void transcribeUtterance(blob, context, durationMs, preRollSnapshot)
    }
    rec.onerror = () => {
      if (!context.closed) context.onError?.('media-recorder-error')
    }

    rec.start()
  }

  const effectiveGates = () => ({
    speech: Math.max(gates.start, noiseFloor + NOISE_MARGIN),
    arm: Math.max(gates.stop, noiseFloor + NOISE_MARGIN / 2),
  })

  const observeLevel = (level: number, now: number) => {
    const gate = effectiveGates()
    if (level > verdict.peak) verdict.peak = level
    if (level >= gate.speech) verdict.confirmed = true

    // What keeps the recording alive must be the SPEECH gate, not the arm gate — arm sits right where
    // room noise lives, so using it renewed lastVoiceAt on every noise tick and the recorder only ever
    // stopped at the 45s ceiling, producing 10-20s windows that Whisper hallucinated from.
    if (level >= gate.speech) lastVoiceAt = now
  }

  const vadTimer = setInterval(() => {
    if (context.closed || paused) return
    const level = meter.level()
    const now = performance.now()

    publishMicLevel({ level, hearing: level >= effectiveGates().arm })

    noiseFloor = level < noiseFloor ? level : Math.min(MAX_NOISE_FLOOR, noiseFloor + NOISE_FLOOR_RISE_PER_TICK)

    if (!recorder) {
      if (level >= effectiveGates().arm) beginUtterance()
      return
    }

    observeLevel(level, now)

    const silentFor = now - lastVoiceAt
    const elapsed = now - utteranceStart
    if (shouldFinishUtterance(silentFor, elapsed, hangoverMs, vadEndpointingEnabled)) {
      finishUtterance()
    }
  }, VAD_POLL_MS)

  rendererLog.info('[STT] escuta iniciada', {
    lang,
    deviceId: deviceId ?? 'default',
    speechGate: gates.start,
    armGate: gates.stop,
  })

  const stop = () => {
    if (context.closed) return
    context.closed = true
    clearInterval(vadTimer)
    finishUtterance()
    resetMicLevel()
    preRollTapCancelled = true
    stopPreRollTap?.()

    meter.close()
  }

  const setPaused = (next: boolean) => {
    if (context.closed || paused === next) return
    paused = next
    if (next) {
      finishUtterance()
      resetMicLevel()
    }
    for (const track of stream.getAudioTracks()) track.enabled = !next

    if (!next) noiseFloor = 1
    rendererLog.debug('[STT] captura', { estado: next ? 'pausada' : 'retomada' })
  }

  return { stop, setPaused }
}
