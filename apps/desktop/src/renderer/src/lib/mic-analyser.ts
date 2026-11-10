function getAudioConstraints(deviceId?: string): MediaTrackConstraints | boolean {
  const aec: MediaTrackConstraints = { echoCancellation: true, noiseSuppression: true }
  return deviceId && deviceId !== 'default' ? { ...aec, deviceId: { exact: deviceId } } : aec
}

export function getUserMediaWithFallback(deviceId?: string): Promise<MediaStream> {
  const constraints = getAudioConstraints(deviceId)
  return navigator.mediaDevices.getUserMedia({ audio: constraints }).catch((error: unknown) => {
    const name = error instanceof Error ? error.name : ''
    if (constraints === true || (name !== 'OverconstrainedError' && name !== 'NotFoundError')) throw error
    return navigator.mediaDevices.getUserMedia({ audio: true })
  })
}

type MicLevelMeter = {
  level: () => number
  close: () => void
}

export function createMicLevelMeter(stream: MediaStream): MicLevelMeter {
  const audioCtx = new AudioContext()
  const analyser = audioCtx.createAnalyser()
  analyser.fftSize = 512
  analyser.smoothingTimeConstant = 0.6
  const freqData = new Uint8Array(analyser.frequencyBinCount)
  audioCtx.createMediaStreamSource(stream).connect(analyser)

  return {
    level: () => {
      analyser.getByteFrequencyData(freqData)
      let sum = 0
      for (let i = 2; i < 40; i++) sum += freqData[i] ?? 0
      return Math.min(1, sum / 38 / 80)
    },

    close: () => {
      for (const track of stream.getTracks()) track.stop()
      audioCtx.close().catch(() => {})
    },
  }
}
