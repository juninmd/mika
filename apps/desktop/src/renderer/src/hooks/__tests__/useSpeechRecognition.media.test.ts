import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { subscribeLiveTranscript } from '../../lib/live-transcript-bus'
import { startMediaRecorderTranscription } from '../useSpeechRecognition.media'

const transcribeAudio = vi.fn()
const getElectronApiMock = vi.hoisted(() => vi.fn())

vi.mock('../../lib/shell-api', () => ({ getElectronApi: getElectronApiMock }))

class MockMediaRecorder {
  mimeType = 'audio/webm'
  ondataavailable: ((event: { data: Blob }) => void) | null = null
  onerror: (() => void) | null = null

  constructor(public readonly stream: MediaStream) {}

  start(): void {}
  stop(): void {}
}

class MockAnalyser {
  fftSize = 512
  smoothingTimeConstant = 0.6
  frequencyBinCount = 256
  getByteFrequencyData(): void {}
}

class MockAudioContext {
  createAnalyser(): MockAnalyser {
    return new MockAnalyser()
  }
  createMediaStreamSource(): { connect: () => void } {
    return { connect: () => {} }
  }
  close(): Promise<void> {
    return Promise.resolve()
  }
}

describe('startMediaRecorderTranscription', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    transcribeAudio.mockReset()
    transcribeAudio.mockResolvedValue({ success: true, data: { text: '' } })
    getElectronApiMock.mockReset()
    getElectronApiMock.mockReturnValue({ runtime: { transcribeAudio, log: vi.fn(async () => {}) } })

    Object.defineProperty(globalThis, 'MediaRecorder', {
      value: MockMediaRecorder,
      configurable: true,
    })

    Object.defineProperty(globalThis, 'AudioContext', {
      value: MockAudioContext,
      configurable: true,
    })

    Object.defineProperty(globalThis, 'navigator', {
      value: {
        mediaDevices: {
          getUserMedia: vi.fn(async () => ({
            getTracks: () => [{ stop: vi.fn() }],
          })),
        },
      },
      configurable: true,
    })
  })

  it('captures audio from the selected input device', async () => {
    const session = await startMediaRecorderTranscription({
      lang: 'pt-BR',
      deviceId: 'selected-mic-id',
      onTranscript: vi.fn(),
    })

    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith({
      audio: expect.objectContaining({ deviceId: { exact: 'selected-mic-id' } }),
    })

    session.stop()
  })

  it('falls back to the default device when the saved deviceId no longer exists', async () => {
    const overconstrained = new Error('')
    overconstrained.name = 'OverconstrainedError'
    const getUserMedia = vi
      .fn()
      .mockRejectedValueOnce(overconstrained)
      .mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] })
    Object.defineProperty(globalThis, 'navigator', {
      value: { mediaDevices: { getUserMedia } },
      configurable: true,
    })

    const session = await startMediaRecorderTranscription({
      lang: 'pt-BR',
      deviceId: 'stale-mic-id',
      onTranscript: vi.fn(),
    })

    expect(getUserMedia).toHaveBeenNthCalledWith(1, {
      audio: expect.objectContaining({ deviceId: { exact: 'stale-mic-id' } }),
    })
    expect(getUserMedia).toHaveBeenNthCalledWith(2, { audio: true })

    session.stop()
  })

  it('propagates non-device errors (permission denied) without fallback', async () => {
    const denied = new Error('Permission denied')
    denied.name = 'NotAllowedError'
    const getUserMedia = vi.fn().mockRejectedValue(denied)
    Object.defineProperty(globalThis, 'navigator', {
      value: { mediaDevices: { getUserMedia } },
      configurable: true,
    })

    await expect(
      startMediaRecorderTranscription({ lang: 'pt-BR', deviceId: 'any-mic', onTranscript: vi.fn() }),
    ).rejects.toThrow('Permission denied')
    expect(getUserMedia).toHaveBeenCalledTimes(1)
  })

  // Regression: once getUserMedia resolved, any throw before the session object existed (bridge missing,
  // AudioContext refusing to construct) left the stream open with nobody holding a handle to close it.
  it.each([
    [
      'the bridge lookup throws',
      () =>
        getElectronApiMock.mockImplementation(() => {
          throw new Error('no bridge')
        }),
      'no bridge',
    ],
    [
      'the AudioContext cannot be created',
      () =>
        Object.defineProperty(globalThis, 'AudioContext', {
          value: class {
            constructor() {
              throw new Error('audio ctx blocked')
            }
          },
          configurable: true,
        }),
      'audio ctx blocked',
    ],
  ])('stops the mic tracks when %s after getUserMedia', async (_name, breakIt, message) => {
    const stop = vi.fn()
    const getUserMedia = vi.fn(async () => ({ getTracks: () => [{ stop }] }))
    Object.defineProperty(globalThis, 'navigator', { value: { mediaDevices: { getUserMedia } }, configurable: true })
    breakIt()

    await expect(startMediaRecorderTranscription({ lang: 'pt-BR', onTranscript: vi.fn() })).rejects.toThrow(message)
    expect(stop).toHaveBeenCalled()
  })

  it('closes the pre-roll AudioContext when the worklet module fails to load', async () => {
    const close = vi.fn(async () => {})
    const addModule = vi.fn().mockRejectedValue(new Error('worklet 404'))
    vi.stubGlobal('window', { location: { href: 'http://localhost/' } })
    Object.defineProperty(globalThis, 'AudioContext', {
      value: class extends MockAudioContext {
        audioWorklet = { addModule }
        close = close
      },
      configurable: true,
    })

    const session = await startMediaRecorderTranscription({ lang: 'pt-BR', onTranscript: vi.fn() })
    await vi.waitFor(() => expect(addModule).toHaveBeenCalled())
    // The meter's own context is closed by session.stop(); the failed tap's context must close on its own.
    await vi.waitFor(() => expect(close).toHaveBeenCalledTimes(1))
    session.stop()
    vi.unstubAllGlobals()
  })

  // End of speech used to wait the whole hangover (`voice.vadSilenceMs`, 1200 ms) and only THEN start
  // decoding + STT. Transcribing at a third of the hangover hides the STT time inside the wait.
  describe('speculative transcription during the hangover', () => {
    let level = 0
    let workletPort: { onmessage: ((event: MessageEvent) => void) | null } | null = null

    const pushAudio = (samples: number) =>
      workletPort?.onmessage?.({ data: { type: 'audio', audio: new Float32Array(samples).fill(0.2) } } as MessageEvent)
    const sentBytes = (call: number) => transcribeAudio.mock.calls[call]?.[0].audio.byteLength

    beforeEach(() => {
      vi.useFakeTimers()
      level = 0
      workletPort = null
      vi.stubGlobal('window', { location: { href: 'http://localhost/' } })
      Object.defineProperty(globalThis, 'MediaRecorder', {
        value: class extends MockMediaRecorder {
          onstop: (() => void) | null = null
          override stop(): void {
            this.ondataavailable?.({ data: new Blob(['x']) })
            this.onstop?.()
          }
        },
        configurable: true,
      })
      Object.defineProperty(globalThis, 'AudioContext', {
        value: class extends MockAudioContext {
          sampleRate = 48_000
          audioWorklet = { addModule: async () => {} }
          override createAnalyser(): MockAnalyser {
            const analyser = new MockAnalyser()
            analyser.getByteFrequencyData = (data?: Uint8Array) => data?.fill(Math.round(level * 80))
            return analyser
          }
        },
        configurable: true,
      })
      vi.stubGlobal(
        'AudioWorkletNode',
        class {
          port = { onmessage: null }
          constructor() {
            workletPort = this.port
          }
          disconnect(): void {}
        },
      )
    })

    afterEach(() => {
      vi.useRealTimers()
      vi.unstubAllGlobals()
    })

    async function speak(ms: number) {
      level = 0.6
      for (let t = 0; t < ms; t += 5) {
        pushAudio(80)
        await vi.advanceTimersByTimeAsync(5)
      }
    }

    async function silence(ms: number) {
      level = 0
      for (let t = 0; t < ms; t += 5) {
        pushAudio(80)
        await vi.advanceTimersByTimeAsync(5)
      }
    }

    it('transcribes before the hangover ends and reuses that transcript', async () => {
      transcribeAudio.mockResolvedValue({ success: true, data: { text: 'oi mika' } })
      const onTranscript = vi.fn()
      const session = await startMediaRecorderTranscription({ lang: 'pt-BR', vadSilenceMs: 300, onTranscript })
      await vi.waitFor(() => expect(workletPort).not.toBeNull())
      await silence(50)
      await speak(200)

      await silence(150)
      expect(transcribeAudio).toHaveBeenCalledTimes(1)
      expect(onTranscript).not.toHaveBeenCalled()

      await silence(250)
      expect(onTranscript).toHaveBeenCalledWith('oi mika', expect.anything())
      expect(transcribeAudio).toHaveBeenCalledTimes(1)
      session.stop()
    })

    it('discards the early transcript when the user speaks again', async () => {
      transcribeAudio
        .mockResolvedValueOnce({ success: true, data: { text: 'mika' } })
        .mockResolvedValueOnce({ success: true, data: { text: 'mika, abre o youtube' } })
      const onTranscript = vi.fn()
      const session = await startMediaRecorderTranscription({ lang: 'pt-BR', vadSilenceMs: 300, onTranscript })
      await vi.waitFor(() => expect(workletPort).not.toBeNull())
      await silence(50)
      await speak(200)
      await silence(150)
      await speak(200)
      await silence(400)

      expect(transcribeAudio).toHaveBeenCalledTimes(2)
      expect(sentBytes(1)).toBeGreaterThan(sentBytes(0))
      expect(onTranscript).toHaveBeenCalledTimes(1)
      expect(onTranscript).toHaveBeenCalledWith('mika, abre o youtube', expect.anything())
      session.stop()
    })

    // The open chat shows the sentence growing while it is spoken, then hands over to the real turn.
    it('streams partials to the open chat and clears them once the turn is delivered', async () => {
      transcribeAudio
        .mockResolvedValueOnce({ success: true, data: { text: 'mika abre' } })
        .mockResolvedValue({ success: true, data: { text: 'mika abre o youtube' } })
      const seen: Array<string | null> = []
      const unsubscribe = subscribeLiveTranscript((text) => seen.push(text))
      const onTranscript = vi.fn()
      const session = await startMediaRecorderTranscription({
        lang: 'pt-BR',
        vadSilenceMs: 300,
        livePartials: true,
        onTranscript,
      })
      await vi.waitFor(() => expect(workletPort).not.toBeNull())
      await silence(50)
      await speak(1300)
      await silence(400)

      expect(seen).toEqual([null, '', 'mika abre', null])
      expect(onTranscript).toHaveBeenCalledWith('mika abre o youtube', expect.anything())
      unsubscribe()
      session.stop()
    })
  })
})
