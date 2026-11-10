import { useCallback, useEffect, useRef, useState } from 'react'
import { AUDIO_INPUT_DEVICE_KEY } from '../lib/audio-storage-keys'
import { createMicLevelMeter, getUserMediaWithFallback } from '../lib/mic-analyser'
import { createGenerationGuard, openMicIfCurrent, stopStream } from '../lib/mic-stream'
import { readLocalStorage } from '../lib/safe-local-storage'

const TICK_MS = 100

type Meter = ReturnType<typeof createMicLevelMeter>

/**
 * Live mic level on the SAME scale the speech gate reads (`createMicLevelMeter`, used by
 * `useSpeechRecognition.media.ts`) — the Audio devices test uses a whole-spectrum average instead,
 * which would put a calibrated threshold line in the wrong place. Opens the mic chosen in Audio
 * devices; `sample(ms)` collects the ticks of a window for calibration.
 */
export function useMicLevelMeter() {
  const [level, setLevel] = useState(0)
  const [running, setRunning] = useState(false)
  const [failed, setFailed] = useState(false)
  const meterRef = useRef<Meter | null>(null)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const samplesRef = useRef<number[] | null>(null)
  // stop()/unmount invalidates a start() still awaiting getUserMedia; `pendingRef` dedupes concurrent starts.
  const guardRef = useRef(createGenerationGuard())
  const pendingRef = useRef<Promise<boolean> | null>(null)

  const stop = useCallback(() => {
    guardRef.current.invalidate()
    pendingRef.current = null
    if (timerRef.current) clearInterval(timerRef.current)
    timerRef.current = null
    meterRef.current?.close()
    meterRef.current = null
    setRunning(false)
    setLevel(0)
  }, [])

  const open = useCallback(async (token: number): Promise<boolean> => {
    const { isCurrent } = guardRef.current
    let stream: MediaStream | null = null
    try {
      const deviceId = readLocalStorage(AUDIO_INPUT_DEVICE_KEY) ?? undefined
      stream = await openMicIfCurrent(
        () => getUserMediaWithFallback(deviceId),
        () => isCurrent(token),
      )
      if (!stream) return false
      const meter = createMicLevelMeter(stream)
      meterRef.current = meter
      timerRef.current = setInterval(() => {
        const value = meter.level()
        setLevel(value)
        samplesRef.current?.push(value)
      }, TICK_MS)
      setFailed(false)
      setRunning(true)
      return true
    } catch {
      stopStream(stream)
      if (isCurrent(token)) setFailed(true)
      return false
    }
  }, [])

  const start = useCallback((): Promise<boolean> => {
    if (meterRef.current) return Promise.resolve(true)
    if (pendingRef.current) return pendingRef.current
    const pending = open(guardRef.current.begin()).finally(() => {
      if (pendingRef.current === pending) pendingRef.current = null
    })
    pendingRef.current = pending
    return pending
  }, [open])

  const sample = useCallback(
    async (ms: number): Promise<number[]> => {
      if (!(await start())) return []
      samplesRef.current = []
      await new Promise((resolve) => setTimeout(resolve, ms))
      const collected = samplesRef.current ?? []
      samplesRef.current = null
      return collected
    },
    [start],
  )

  useEffect(() => stop, [stop])

  return { level, running, failed, start, stop, sample }
}
