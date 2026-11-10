import { t } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { useEffect, useRef, useState } from 'react'
import { blobToPcm16, bytesToBase64, PCM_MIME_TYPE } from '../../hooks/speech-transcribe'
import { AUDIO_INPUT_DEVICE_KEY } from '../../lib/audio-storage-keys'
import { getUserMediaWithFallback } from '../../lib/mic-analyser'
import { createGenerationGuard, openMicIfCurrent } from '../../lib/mic-stream'
import { readLocalStorage } from '../../lib/safe-local-storage'
import { getElectronApi } from '../../lib/shell-api'
import { getAverageVolume, handleVadState, teardownVadResources, type VadState } from './homologation-vad-audio'

let vadEntrySeq = 0

export function useHomologationVad() {
  const [vadActive, setVadActive] = useState(false)
  const [audioLevel, setAudioLevel] = useState(0)
  const [vadSpeaking, setVadSpeaking] = useState(false)
  const [vadHistory, setVadHistory] = useState<Array<{ id: string; text: string }>>([])
  const [vadThreshold, setVadThreshold] = useState(0.15)
  const [vadStatus, setVadStatus] = useState(() => t('settings.homologation.vad.status.idle'))
  const [sttText, setSttText] = useState('')
  const [sttLoading, setSttLoading] = useState(false)

  const micStreamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const animationFrameRef = useRef<number | null>(null)
  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const recorderChunksRef = useRef<Blob[]>([])
  const vadThresholdRef = useRef(vadThreshold)
  const stoppedByUserRef = useRef(false)
  // stop/unmount/restart retires a startVadTest still awaiting getUserMedia (its stream is closed on arrival).
  const guardRef = useRef(createGenerationGuard())

  const processRecordedAudio = async (chunks: Blob[], mimeType: string) => {
    setSttLoading(true)
    setVadStatus(t('settings.homologation.vad.status.sending'))
    try {
      // The backend takes PCM16 only (AGENTS.md §2): the recorder's webm/ogg would be refused by local STT.
      const pcm = await blobToPcm16(new Blob(chunks, { type: mimeType }))

      const api = getElectronApi()
      const res = await api.runtime.transcribeAudio({
        audioBase64: bytesToBase64(pcm),
        mimeType: PCM_MIME_TYPE,
      })

      if (res.success && res.data?.text) {
        setSttText(res.data.text)
        setVadStatus(t('settings.homologation.vad.status.transcribed'))
        setVadHistory((prev) => [{ id: `vad-${++vadEntrySeq}`, text: res.data?.text ?? '' }, ...prev.slice(0, 4)])
      } else if (res.success) {
        setVadStatus(t('settings.homologation.vad.status.tooShort'))
      } else {
        setVadStatus(
          t('settings.homologation.vad.status.sttError', {
            error: res.error || t('settings.homologation.vad.status.noResponse'),
          }),
        )
      }
    } catch (err) {
      setVadStatus(t('settings.homologation.vad.status.sendError', { error: toDisplayError(err, String(err)) }))
    } finally {
      setSttLoading(false)
    }
  }

  const startVadTest = async () => {
    const token = guardRef.current.begin()
    try {
      stoppedByUserRef.current = false
      // Must listen to the SAME mic Mika uses; defaulting to the system mic passed green on the webcam mic for headset users.
      const stream = await openMicIfCurrent(
        () => getUserMediaWithFallback(readLocalStorage(AUDIO_INPUT_DEVICE_KEY) ?? undefined),
        () => guardRef.current.isCurrent(token),
      )
      if (!stream) return
      micStreamRef.current = stream
      setVadActive(true)
      setVadStatus(t('settings.homologation.vad.status.listening'))

      const audioCtx = new AudioContext()
      audioContextRef.current = audioCtx

      const source = audioCtx.createMediaStreamSource(stream)
      const analyser = audioCtx.createAnalyser()
      analyser.fftSize = 512
      source.connect(analyser)
      analyserRef.current = analyser

      const bufferLength = analyser.frequencyBinCount
      const dataArray = new Uint8Array(bufferLength)

      const mediaRecorder = new MediaRecorder(stream)
      mediaRecorderRef.current = mediaRecorder
      recorderChunksRef.current = []

      mediaRecorder.ondataavailable = (e) => {
        if (e.data.size > 0) {
          recorderChunksRef.current.push(e.data)
        }
      }

      mediaRecorder.onstop = () => {
        const chunks = recorderChunksRef.current
        recorderChunksRef.current = []
        if (stoppedByUserRef.current) {
          return
        }
        void processRecordedAudio(chunks, mediaRecorder.mimeType)
      }

      let currentVadState: VadState = {
        speakingStreak: 0,
        silenceStreak: 0,
        isSpeaking: false,
      }

      const updateLevel = () => {
        if (!analyserRef.current || audioCtx.state === 'closed') return
        analyserRef.current.getByteFrequencyData(dataArray)

        const normalized = getAverageVolume(dataArray) / 255
        setAudioLevel(normalized)

        currentVadState = handleVadState(
          normalized,
          vadThresholdRef.current,
          currentVadState,
          () => {
            setVadSpeaking(true)
            setVadStatus(t('settings.homologation.vad.status.speaking'))
            recorderChunksRef.current = []
            try {
              mediaRecorder.start()
            } catch (err) {
              setVadStatus(
                t('settings.homologation.vad.status.recordStartError', { error: toDisplayError(err, String(err)) }),
              )
            }
          },
          () => {
            setVadSpeaking(false)
            setVadStatus(t('settings.homologation.vad.status.silence'))
            try {
              mediaRecorder.stop()
            } catch (err) {
              setVadStatus(
                t('settings.homologation.vad.status.recordStopError', { error: toDisplayError(err, String(err)) }),
              )
            }
          },
        )

        animationFrameRef.current = requestAnimationFrame(updateLevel)
      }

      animationFrameRef.current = requestAnimationFrame(updateLevel)
    } catch (err) {
      // Everything opened before the failure (mic stream, AudioContext, MediaRecorder) must be released.
      teardownVadResources({ animationFrameRef, mediaRecorderRef, audioContextRef }, micStreamRef.current)
      micStreamRef.current = null
      if (!guardRef.current.isCurrent(token)) return
      setVadActive(false)
      setVadStatus(t('settings.homologation.vad.status.testError', { error: toDisplayError(err, String(err)) }))
    }
  }

  const stopVadTest = () => {
    guardRef.current.invalidate()
    stoppedByUserRef.current = true
    setVadActive(false)
    setVadSpeaking(false)
    setAudioLevel(0)
    setVadStatus(t('settings.homologation.vad.status.finished'))
    teardownVadResources({ animationFrameRef, mediaRecorderRef, audioContextRef }, micStreamRef.current)
    micStreamRef.current = null
  }

  // Cleanup must only run on real unmount, hence empty deps + refs (not state) for these values.
  useEffect(() => {
    const guard = guardRef.current
    return () => {
      guard.invalidate()
      stoppedByUserRef.current = true
      teardownVadResources({ animationFrameRef, mediaRecorderRef, audioContextRef }, micStreamRef.current)
    }
  }, [])

  return {
    vadActive,
    audioLevel,
    vadSpeaking,
    vadHistory,
    vadThreshold,
    setVadThreshold,
    vadStatus,
    sttText,
    sttLoading,
    vadThresholdRef,
    startVadTest,
    stopVadTest,
  }
}
