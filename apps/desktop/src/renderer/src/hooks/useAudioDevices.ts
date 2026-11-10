import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { AUDIO_INPUT_DEVICE_KEY, AUDIO_OUTPUT_DEVICE_KEY } from '../lib/audio-storage-keys'
import { getUserMediaWithFallback } from '../lib/mic-analyser'
import { createGenerationGuard, openMicIfCurrent, stopStream } from '../lib/mic-stream'
import { rendererLog } from '../lib/renderer-logger'
import { readLocalStorage, writeLocalStorage } from '../lib/safe-local-storage'
import { useUIStore } from '../store/useUIStore'

const MIC_LEVEL_PUSH_INTERVAL_MS = 100

type MicPermissionState = 'unknown' | 'granted' | 'denied' | 'not-found' | 'device-error'

/**
 * Opens the mic and an analyser on it. `null` when the session went stale while `getUserMedia` was pending
 * (the stream is already stopped); anything opened before a failure is released before rethrowing.
 */
async function openMonitor(deviceId: string, isCurrent: () => boolean) {
  const stream = await openMicIfCurrent(() => getUserMediaWithFallback(deviceId), isCurrent)
  if (!stream) return null
  let ctx: AudioContext | undefined
  try {
    ctx = new AudioContext()
    const analyser = ctx.createAnalyser()
    analyser.fftSize = 256
    ctx.createMediaStreamSource(stream).connect(analyser)
    return { stream, ctx, analyser }
  } catch (error) {
    stopStream(stream)
    ctx?.close().catch(() => {})
    throw error
  }
}

/** ~10 Hz visual meter: re-rendering the whole panel at 60 Hz for a bar is wasted work. */
function startLevelLoop(
  analyser: AnalyserNode,
  setLevel: (level: number) => void,
  frame: { current: number | null },
): void {
  const data = new Uint8Array(analyser.frequencyBinCount)
  let lastPushAt = 0
  const tick = (now: number) => {
    frame.current = requestAnimationFrame(tick)
    if (now - lastPushAt < MIC_LEVEL_PUSH_INTERVAL_MS) return
    lastPushAt = now
    analyser.getByteFrequencyData(data)
    let sum = 0
    for (const v of data) sum += v
    setLevel(Math.min(100, (sum / data.length / 90) * 100))
  }
  frame.current = requestAnimationFrame(tick)
}

function classifyMicError(error: unknown): MicPermissionState {
  const name = error instanceof Error ? error.name : undefined
  if (name === 'NotAllowedError' || name === 'PermissionDeniedError') return 'denied'
  if (name === 'NotFoundError' || name === 'DevicesNotFoundError') return 'not-found'
  return 'device-error'
}

interface AudioDevice {
  deviceId: string
  label: string
  kind: 'audioinput' | 'audiooutput'
}

export function useAudioDevices(onSave?: (path: string, value: unknown) => Promise<void>) {
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [selectedInputId, setSelectedInputIdState] = useState(
    () => readLocalStorage(AUDIO_INPUT_DEVICE_KEY) ?? 'default',
  )
  const [selectedOutputId, setSelectedOutputIdState] = useState(
    () => readLocalStorage(AUDIO_OUTPUT_DEVICE_KEY) ?? 'default',
  )
  const [micLevel, setMicLevel] = useState(0)
  const [isMonitoring, setIsMonitoring] = useState(false)
  const [permissionState, setPermissionState] = useState<MicPermissionState>('unknown')

  const animRef = useRef<number | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const audioCtxRef = useRef<AudioContext | null>(null)
  // stopMonitoring()/unmount/a newer start retires a startMonitoring still awaiting getUserMedia.
  const guardRef = useRef(createGenerationGuard())

  const enumerate = useCallback(async () => {
    try {
      const all = await navigator.mediaDevices.enumerateDevices()
      setDevices(
        all
          .filter(
            (d): d is MediaDeviceInfo & { kind: 'audioinput' | 'audiooutput' } =>
              d.kind === 'audioinput' || d.kind === 'audiooutput',
          )
          .map((d) => ({
            deviceId: d.deviceId,
            label:
              d.label ||
              t(
                d.kind === 'audioinput' ? 'settings.audioDevices.fallbackMic' : 'settings.audioDevices.fallbackSpeaker',
              ),
            kind: d.kind,
          })),
      )
    } catch (error) {
      // Sem isto, a lista de microfones aparecia vazia sem nenhum rastro do motivo.
      rendererLog.warn('[audio] falha ao enumerar dispositivos', { error: toErrorMessage(error) })
    }
  }, [])

  const requestPermission = useCallback(async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      stream.getTracks().forEach((t) => {
        t.stop()
      })
      setPermissionState('granted')
      await enumerate()
    } catch (err) {
      setPermissionState(classifyMicError(err))
    }
  }, [enumerate])

  const stopMonitoring = useCallback(() => {
    guardRef.current.invalidate()
    if (animRef.current) cancelAnimationFrame(animRef.current)
    stopStream(streamRef.current)
    audioCtxRef.current?.close().catch(() => {})
    streamRef.current = null
    audioCtxRef.current = null
    setIsMonitoring(false)
    setMicLevel(0)
  }, [])

  const startMonitoring = useCallback(
    async (overrideDeviceId?: string) => {
      if (isMonitoring && !overrideDeviceId) {
        stopMonitoring()
        return
      }
      if (isMonitoring && overrideDeviceId) {
        stopMonitoring()
      }
      const token = guardRef.current.begin()
      try {
        const monitor = await openMonitor(overrideDeviceId ?? selectedInputId, () => guardRef.current.isCurrent(token))
        if (!monitor) return
        streamRef.current = monitor.stream
        audioCtxRef.current = monitor.ctx
        setIsMonitoring(true)
        startLevelLoop(monitor.analyser, setMicLevel, animRef)
      } catch (err) {
        if (guardRef.current.isCurrent(token)) setPermissionState(classifyMicError(err))
      }
    },
    [isMonitoring, selectedInputId, stopMonitoring],
  )

  const setSelectedInputId = useCallback(
    async (id: string) => {
      const previousId = selectedInputId
      setSelectedInputIdState(id)
      writeLocalStorage(AUDIO_INPUT_DEVICE_KEY, id)
      try {
        await onSave?.('audio.inputDeviceId', id)
      } catch (err) {
        setSelectedInputIdState(previousId)
        writeLocalStorage(AUDIO_INPUT_DEVICE_KEY, previousId)
        useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.audioDevices.saveMicFailed')))
        return
      }
      if (isMonitoring) {
        void startMonitoring(id)
      }
    },
    [isMonitoring, onSave, selectedInputId, startMonitoring],
  )

  const setSelectedOutputId = useCallback(
    async (id: string) => {
      const previousId = selectedOutputId
      setSelectedOutputIdState(id)
      writeLocalStorage(AUDIO_OUTPUT_DEVICE_KEY, id)
      try {
        await onSave?.('audio.outputDeviceId', id)
      } catch (err) {
        setSelectedOutputIdState(previousId)
        writeLocalStorage(AUDIO_OUTPUT_DEVICE_KEY, previousId)
        useUIStore
          .getState()
          .addNotification('error', toDisplayError(err, t('settings.audioDevices.saveSpeakerFailed')))
      }
    },
    [onSave, selectedOutputId],
  )

  useEffect(() => {
    void enumerate()
    navigator.mediaDevices.addEventListener('devicechange', enumerate)
    return () => {
      navigator.mediaDevices.removeEventListener('devicechange', enumerate)
      stopMonitoring()
    }
  }, [enumerate, stopMonitoring])

  const inputs = useMemo(() => devices.filter((d) => d.kind === 'audioinput'), [devices])
  const outputs = useMemo(() => devices.filter((d) => d.kind === 'audiooutput'), [devices])

  return {
    inputs,
    outputs,
    selectedInputId,
    selectedOutputId,
    setSelectedInputId,
    setSelectedOutputId,
    micLevel,
    isMonitoring,
    permissionState,
    requestPermission,
    startMonitoring,
    stopMonitoring,
  }
}
