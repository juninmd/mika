import { beforeEach, describe, expect, it, vi } from 'vitest'
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
    getElectronApiMock.mockReturnValue({ runtime: { transcribeAudio } })

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
})
