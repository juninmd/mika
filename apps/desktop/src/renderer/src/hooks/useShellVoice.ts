import { t } from '@mika/sdk/i18n'
import { useEffect, useRef } from 'react'
import type { AppStatus, MessageSource } from '../components/app/app-shell.types'
import { isDuplicateOfRecentInterruption, type RecentInterruption } from '../lib/interruption-dedupe'
import { rendererLog } from '../lib/renderer-logger'
import { decideSpokenTurn } from '../lib/speech-turn'
import { isFatalSpeechError, speechErrorMessageKey } from './shell-commands'
import { useSpeechRecognition } from './useSpeechRecognition'

type NotifyFn = (type: 'info' | 'success' | 'warning' | 'error', message: string, title?: string) => void

type SilenceCommandApi = {
  runtime: {
    checkSilenceCommand: (payload: { text: string }) => Promise<{ matched: boolean; muted?: boolean }>
    // Optional: older bridges/tests may not stub this listener — same shape as onForceStopTts et al.
    onInterruption?: (
      callback: (payload: { transcript?: string; is_interruption?: boolean; timestamp?: number }) => void,
    ) => () => void
  }
}

type Params = {
  eAPI: SilenceCommandApi
  voiceLang: string
  asrProvider: string
  inputDeviceId: string | undefined
  vadThreshold: number
  vadSilenceMs: number
  vadEndpointingEnabled: boolean
  offlineMode: boolean
  micEnabled: boolean
  setMicEnabled: (v: boolean | ((prev: boolean) => boolean)) => void
  appStatus: AppStatus
  ttsIsPlaying: boolean
  stopTTS: () => void
  isSendingRef: { current: boolean }
  holdSpeech: (text: string) => void
  sendMessage: (text: string, source?: MessageSource) => Promise<void>
  addNotification: NotifyFn
}

/** Wires STT: transcript-to-turn decisions, error surfacing, and the mic-enabled start/stop lifecycle. */
export function useShellVoice({
  eAPI,
  voiceLang,
  asrProvider,
  inputDeviceId,
  vadThreshold,
  vadSilenceMs,
  vadEndpointingEnabled,
  offlineMode,
  micEnabled,
  setMicEnabled,
  appStatus,
  ttsIsPlaying,
  stopTTS,
  isSendingRef,
  holdSpeech,
  sendMessage,
  addNotification,
}: Params) {
  // Barge-in double-reply guard (defects.md "barge-in", item 1; see interruption-dedupe.ts). The
  // Rust VAD path answers an interruption faster than this hook's own silence-hangover endpointing
  // ever can, so when both raced for the SAME utterance, the fix is here: defer to the most recent
  // `voice:interruption`'s own transcript, never merely because one landed recently (a wall-clock
  // window alone would drop a genuinely new thing the user says right after a barge-in — code
  // review, blocker). Payloads without a transcript (the silence-endpoint ping, `is_interruption`
  // absent) never got a spoken reply from Rust, so they never overwrite this ref.
  const lastInterruptionRef = useRef<RecentInterruption | null>(null)
  useEffect(() => {
    const off = eAPI.runtime.onInterruption?.((payload) => {
      if (!payload.is_interruption || !payload.transcript) return
      lastInterruptionRef.current = { transcript: payload.transcript, atMs: payload.timestamp ?? Date.now() }
    })
    return off
  }, [eAPI])

  // The busy/hold turn decision, unchanged from before the silence-command fast path — extracted so
  // `onTranscript` can gate it behind `checkSilenceCommand` without nesting the whole thing.
  const handleSpokenTurn = (spoken: string, meta?: { durationMs?: number }) => {
    // busy must be isSendingRef, not appStatus — a stuck status once locked the queue permanently.
    const decision = decideSpokenTurn({
      text: spoken,
      durationMs: meta?.durationMs,
      busy: isSendingRef.current,
    })

    if (decision === 'too-short') {
      rendererLog.debug('[STT] fala curta demais para virar turno', { chars: spoken.length })
      return
    }

    if (decision === 'implausible') {
      rendererLog.info('[STT] transcrição não cabe no áudio — alucinação do Whisper, não vira turno', {
        texto: spoken,
        chars: spoken.length,
        durationMs: meta?.durationMs,
      })
      return
    }

    window.dispatchEvent(new CustomEvent('stt-transcript', { detail: spoken }))

    if (isDuplicateOfRecentInterruption(lastInterruptionRef.current, spoken, Date.now())) {
      rendererLog.info('[STT] fala já respondida pela interrupção do barge-in — turno duplicado descartado', {
        chars: spoken.length,
      })
      return
    }

    if (decision === 'hold') {
      holdSpeech(spoken)
      rendererLog.info('[STT] fala guardada — respondo quando o turno atual terminar', { chars: spoken.length })
      return
    }

    if (appStatus === 'speaking' || ttsIsPlaying) {
      stopTTS()
    }
    void sendMessage(spoken, 'voice')
  }

  const {
    listening: vadIsSpeaking,
    supported: speechSupported,
    start: startSpeechRec,
    stop: stopSpeechRec,
    setPaused: setSpeechPaused,
  } = useSpeechRecognition({
    lang: voiceLang,
    asrProvider,
    deviceId: inputDeviceId,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
    offlineMode,
    onTranscript: (text, meta) => {
      const spoken = text.trim()
      if (!spoken) return

      // Silence-by-voice (specs/08-gamer-companion.md, "Silêncio é feature"): checked BEFORE the
      // busy/hold decision, and even while a turn is in flight — "cala a boca" has to work no
      // matter what the Mika is doing, same guarantee as the Ctrl+Shift+S shortcut. A match already
      // muted/unmuted on the Rust side (same `MuteSpeech` bus event); this utterance must NEVER
      // reach `sendMessage`/the LLM.
      void eAPI.runtime
        .checkSilenceCommand({ text: spoken })
        .catch(() => ({ matched: false }) as const)
        .then((silence) => {
          if (!silence.matched) {
            handleSpokenTurn(spoken, meta)
            return
          }
          window.dispatchEvent(new CustomEvent('stt-transcript', { detail: spoken }))
          if (appStatus === 'speaking' || ttsIsPlaying) {
            stopTTS()
          }
          rendererLog.info('[STT] comando de silêncio — não vira turno de chat', { muted: silence.muted })
        })
    },
    onError: (error) => {
      const key = speechErrorMessageKey(error)
      const message = key ? t(key) : `${t('shell.composer.micError')} ${error}`
      addNotification('error', message, t('shell.neuralWheel.notice.microphone'))

      if (isFatalSpeechError(error)) setMicEnabled(false)
    },
  })

  // Capture reads device/language/VAD settings once, when it opens: they are trigger-only deps here, so a
  // change made in Settings (mic device, thresholds, silence) restarts the capture instead of waiting for the next mic toggle.
  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only deps, see above.
  useEffect(() => {
    if (!micEnabled) {
      stopSpeechRec()
      return
    }

    if (!speechSupported) {
      addNotification(
        'error',
        t(asrProvider === 'browser' ? 'shell.voice.browserSttUnavailable' : 'shell.voice.unsupported'),
        t('shell.neuralWheel.notice.microphone'),
      )
      setMicEnabled(false)
      return
    }

    startSpeechRec()
    return () => {
      stopSpeechRec()
    }
  }, [
    asrProvider,
    micEnabled,
    speechSupported,
    startSpeechRec,
    stopSpeechRec,
    addNotification,
    setMicEnabled,
    inputDeviceId,
    voiceLang,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
  ])

  return { vadIsSpeaking, setSpeechPaused }
}
