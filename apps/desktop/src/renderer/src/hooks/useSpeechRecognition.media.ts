import { hasLiveTranscriptListeners, publishLiveTranscript } from '../lib/live-transcript-bus'
import { createMicLevelMeter, getUserMediaWithFallback } from '../lib/mic-analyser'
import { publishMicLevel, resetMicLevel } from '../lib/mic-level-bus'
import { stopStream } from '../lib/mic-stream'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { logVadSetupFailure, VAD_WORKLET_PATH } from './audio-playback-vad'
import {
  decideEndOfTurn,
  type EndOfTurnCheck,
  lastSmartTurnWindow,
  SMART_TURN_CHECK_TIMEOUT_MS,
  SMART_TURN_MAX_SILENCE_MS,
} from './speech-end-of-turn'
import { createPartialTranscriber } from './speech-partials'
import { createPreRollBuffer, preRollDurationMs } from './speech-preroll'
import {
  capturedPcm16,
  deliverTranscript,
  dropReason,
  logSttMilestone,
  MAX_NOISE_FLOOR,
  type MediaRecorderContext,
  type MediaRecorderFallbackOptions,
  type MediaRecorderSession,
  NOISE_FLOOR_RISE_PER_TICK,
  NOISE_MARGIN,
  PCM_MIME_TYPE,
  requestTranscript,
  SILENCE_HANGOVER_MS,
  scaleVadGates,
  type TranscribeResponse,
  transcribeUtterance,
  VAD_POLL_MS,
} from './speech-transcribe'

export type { SpeechTranscriptHandler } from './speech-transcribe'

type MicMeter = ReturnType<typeof createMicLevelMeter>

/** The utterance being recorded. `pcm` is the worklet tap's audio since the gate opened (`null`
 * without a worklet: then the MediaRecorder blob is decoded instead). `early` is the transcript
 * started during the hangover, valid only while no voice came after `voiceAt`. */
type Utterance = {
  preRoll: Float32Array
  pcm: Float32Array[] | null
  early: { voiceAt: number; result: Promise<TranscribeResponse> } | null
}

/** Fraction of the hangover after which the transcript starts: at 1200 ms, STT runs from 400 ms of
 * silence on, so up to 800 ms of it is hidden inside the wait instead of added after it. Speaking
 * again only costs a discarded STT call. */
const EARLY_TRANSCRIPT_HANGOVER_FRACTION = 1 / 3

/** Silence shorter than this still counts as talking for partials (gaps between words). */
const PARTIAL_VOICE_WINDOW_MS = 300

/** Uma classificação que não volta nesse prazo vira erro e o VAD encerra — nunca prende a escuta. */
function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('smart-turn-timeout')), timeoutMs)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })
}

/** Continuous 16kHz mono tap, same worklet the barge-in path already loads — feeds the pre-roll
 * ring buffer so onset clipping (the gate-opening syllable) doesn't lose audio while nothing is
 * "recording" yet. Best-effort: a webview without AudioWorklet support just runs without pre-roll,
 * same as today. */
async function setupPreRollTap(mediaStream: MediaStream, onAudio: (chunk: Float32Array) => void): Promise<() => void> {
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
    if (type === 'audio' && audio) onAudio(audio)
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
  smartTurnEnabled = false,
  asrProvider,
  livePartials = false,
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
  const maxSilenceMs = Math.max(SMART_TURN_MAX_SILENCE_MS, hangoverMs)

  // Partials skip `requestTranscript`: its latency milestone measures the final transcript only.
  const partials = livePartials
    ? createPartialTranscriber({
        transcribe: (pcm) =>
          runtime.transcribeAudio({
            audio: pcm,
            mimeType: PCM_MIME_TYPE,
            language: lang,
          }) as Promise<TranscribeResponse>,
        publish: publishLiveTranscript,
        enabled: hasLiveTranscriptListeners,
      })
    : null
  const endLiveTranscript = () => {
    if (livePartials) publishLiveTranscript(null)
  }

  const preRoll = createPreRollBuffer()
  let preRollTapCancelled = false
  let stopPreRollTap: (() => void) | null = null
  let tapLive = false
  let current: Utterance | null = null
  const onTapAudio = (chunk: Float32Array) => {
    preRoll.push(chunk)
    current?.pcm?.push(chunk)
  }
  setupPreRollTap(stream, onTapAudio)
    .then((stop) => {
      tapLive = true
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

  // Uma classificação por pausa. `voiceAt` amarra o resultado ao silêncio que o pediu: voz depois
  // disso torna o estado obsoleto e a próxima pausa começa outra checagem.
  let endOfTurnCheck: EndOfTurnCheck = { status: 'idle' }
  let endOfTurnCheckToken = 0

  const finishUtterance = () => {
    if (!recorder) return
    const active = recorder
    recorder = null
    // Invalida qualquer classificação em voo: o `then` tardio não pode mexer na próxima utterance.
    endOfTurnCheckToken += 1
    endOfTurnCheck = { status: 'idle' }
    // Ended by the ceiling or a pause, not by silence: an early transcript may predate the last words.
    if (current?.early && current.early.voiceAt !== lastVoiceAt) current.early = null
    current = null
    partials?.endUtterance()
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
    const utterance: Utterance = { preRoll: preRoll.snapshot(), pcm: tapLive ? [] : null, early: null }
    current = utterance
    endOfTurnCheckToken += 1
    endOfTurnCheck = { status: 'idle' }
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
        endLiveTranscript()
        return
      }
      rendererLog.debug('[STT] enviando utterance', {
        durationMs: Math.round(durationMs),
        early: utterance.early !== null,
        peakLevel: Number(mine.peak.toFixed(3)),
      })
      sendUtterance(utterance, new Blob(chunks, { type: rec.mimeType }), durationMs)
    }
    rec.onerror = () => {
      if (!context.closed) context.onError?.('media-recorder-error')
    }

    rec.start()
  }

  const sendUtterance = (utterance: Utterance, blob: Blob, durationMs: number) => {
    const totalMs = durationMs + preRollDurationMs(utterance.preRoll)
    const delivered = utterance.early
      ? deliverTranscript(utterance.early.result, context, totalMs)
      : utterance.pcm?.length
        ? deliverTranscript(
            requestTranscript(capturedPcm16(utterance.pcm, utterance.preRoll), context),
            context,
            totalMs,
          )
        : transcribeUtterance(blob, context, durationMs, utterance.preRoll)
    void delivered.finally(endLiveTranscript)
  }

  // Voice after the early transcript makes it stale: drop it and let the next pause start another.
  const transcribeEarly = (silentFor: number) => {
    const utterance = current
    if (!utterance?.pcm?.length || !verdict.confirmed) return
    if (utterance.early && utterance.early.voiceAt !== lastVoiceAt) utterance.early = null
    if (utterance.early || !vadEndpointingEnabled || silentFor < hangoverMs * EARLY_TRANSCRIPT_HANGOVER_FRACTION) return
    utterance.early = {
      voiceAt: lastVoiceAt,
      result: requestTranscript(capturedPcm16(utterance.pcm, utterance.preRoll), context),
    }
  }

  // Uma pausa, uma classificação. A chamada é limitada por timeout e o resultado só é aceito se a
  // utterance e o `lastVoiceAt` que a pediu ainda forem os atuais — voz depois da pausa invalida.
  const startSmartTurnCheck = (utterance: Utterance, voiceAt: number) => {
    const token = ++endOfTurnCheckToken
    endOfTurnCheck = { status: 'pending', voiceAt }
    if (typeof runtime.classifyTurnEnd !== 'function') {
      endOfTurnCheck = { status: 'error', voiceAt }
      return
    }

    const pcm = lastSmartTurnWindow(capturedPcm16(utterance.pcm ?? [], utterance.preRoll))
    const startedAt = performance.now()
    const request = Promise.resolve().then(() => runtime.classifyTurnEnd({ audio: pcm, mimeType: PCM_MIME_TYPE }))
    void withTimeout(request, SMART_TURN_CHECK_TIMEOUT_MS)
      .then((result) => {
        if (token !== endOfTurnCheckToken || current !== utterance || lastVoiceAt !== voiceAt) return
        endOfTurnCheck = {
          status: result.complete ? 'complete' : 'incomplete',
          voiceAt,
          probability: result.probability,
          latencyMs: result.latencyMs,
        }
        logSttMilestone(runtime, 'smart-turn', {
          complete: result.complete,
          probability: result.probability,
          latencyMs: result.latencyMs,
          wallMs: Math.round(performance.now() - startedAt),
          reason: result.reason,
        })
      })
      .catch((error: unknown) => {
        if (token !== endOfTurnCheckToken || current !== utterance || lastVoiceAt !== voiceAt) return
        endOfTurnCheck = { status: 'error', voiceAt }
        rendererLog.debug('[STT] smart turn indisponível — encerrando pelo VAD', {
          error: error instanceof Error ? error.message : String(error),
        })
      })
  }

  const tickPartials = (now: number, silentFor: number) => {
    if (!partials || !verdict.confirmed || !current) return
    const { pcm, preRoll: onset } = current
    partials.tick(now, silentFor < PARTIAL_VOICE_WINDOW_MS && Boolean(pcm?.length), () =>
      capturedPcm16(pcm ?? [], onset),
    )
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

  const tickRecording = (level: number, now: number) => {
    observeLevel(level, now)

    const silentFor = now - lastVoiceAt
    const elapsed = now - utteranceStart
    const utterance = current
    const action = decideEndOfTurn({
      silentForMs: silentFor,
      elapsedMs: elapsed,
      hangoverMs,
      maxSilenceMs,
      endpointingEnabled: vadEndpointingEnabled,
      // Sem tap de PCM não há o que classificar: cai no comportamento puramente acústico.
      smartTurnEnabled: smartTurnEnabled && Boolean(utterance?.pcm?.length),
      lastVoiceAt,
      check: endOfTurnCheck,
    })
    if (action === 'finish') {
      finishUtterance()
      return
    }
    if (action === 'check' && utterance?.pcm?.length) startSmartTurnCheck(utterance, lastVoiceAt)
    tickPartials(now, silentFor)
    transcribeEarly(silentFor)
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

    tickRecording(level, now)
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
    endLiveTranscript()
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
