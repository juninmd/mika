import { useEffect, useRef } from 'react'
import { getUserMediaWithFallback } from '../lib/mic-analyser'
import { rendererLog } from '../lib/renderer-logger'
import { createVadMessageHandler, logVadSetupFailure, VAD_WORKLET_PATH } from './audio-playback-vad'

/** Wires the barge-in mic (VAD AudioWorklet) into `ctx` while `vadEnabled` is true. */
export function useBargeInVad(
  getCtx: () => { ctx: AudioContext; analyser: AnalyserNode },
  vadEnabled: boolean,
  inputDeviceId: string | undefined,
  isPlaying: () => boolean,
): void {
  const vadNodeRef = useRef<AudioWorkletNode | null>(null)
  const vadModuleCtxRef = useRef<AudioContext | null>(null)
  // Read through a ref: the worklet handler outlives renders, and a new getter must not rewire the mic.
  const isPlayingRef = useRef(isPlaying)
  isPlayingRef.current = isPlaying

  useEffect(() => {
    if (!vadEnabled) return
    const { ctx } = getCtx()
    let cancelled = false
    let micStream: MediaStream | null = null
    let micSource: MediaStreamAudioSourceNode | null = null

    const stopMic = () => {
      try {
        micSource?.disconnect()
      } catch (e) {
        rendererLog.warn('[VAD] Failed to disconnect mic source:', e)
      }
      for (const track of micStream?.getTracks() ?? []) track.stop()
      micSource = null
      micStream = null
    }
    const setup = async () => {
      if (vadModuleCtxRef.current !== ctx) {
        await ctx.audioWorklet.addModule(VAD_WORKLET_PATH())
        vadModuleCtxRef.current = ctx
      }
      if (cancelled) return

      const vadNode = new AudioWorkletNode(ctx, 'vad-processor', {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        processorOptions: { sampleRate: ctx.sampleRate },
      })
      vadNode.port.onmessage = createVadMessageHandler(() => isPlayingRef.current())
      vadNodeRef.current = vadNode

      // Barge-in mic: must use getUserMediaWithFallback(deviceId), the configured device, never the
      // system default — a mismatch once had the VAD listen to the wrong mic while Mika kept talking.
      micStream = await getUserMediaWithFallback(inputDeviceId)
      if (cancelled) {
        stopMic()
        return
      }
      micSource = ctx.createMediaStreamSource(micStream)
      micSource.connect(vadNode)
      rendererLog.debug('[VAD] AudioWorklet connected')
    }
    setup().catch((err: unknown) => logVadSetupFailure(err, cancelled))

    return () => {
      cancelled = true
      stopMic()
      if (vadNodeRef.current) {
        try {
          vadNodeRef.current.port.onmessage = null
          vadNodeRef.current.disconnect()
        } catch (e) {
          rendererLog.warn('[VAD] Failed to disconnect node:', e)
        }
        vadNodeRef.current = null
      }
    }
  }, [getCtx, vadEnabled, inputDeviceId])
}
