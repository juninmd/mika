import { afterEach, describe, expect, it, vi } from 'vitest'

const sendVadAudio = vi.fn()
vi.mock('../../lib/shell-api', () => ({
  getElectronApi: () => ({ runtime: { sendVadAudio } }),
}))
vi.mock('../../lib/renderer-logger', () => ({
  rendererLog: { debug: vi.fn(), warn: vi.fn(), error: vi.fn(), info: vi.fn() },
}))

import { rendererLog } from '../../lib/renderer-logger'
import { computeAmplitude, createVadMessageHandler, logVadSetupFailure } from '../audio-playback-vad'

afterEach(() => {
  vi.clearAllMocks()
  vi.useRealTimers()
})

describe('computeAmplitude — RMS used to decide barge-in sensitivity', () => {
  it('returns 0 for silence', () => {
    expect(computeAmplitude(new Float32Array([0, 0, 0]))).toBe(0)
  })

  it('returns the RMS of a constant signal', () => {
    expect(computeAmplitude(new Float32Array([0.5, 0.5, 0.5, 0.5]))).toBeCloseTo(0.5)
  })

  it('does not divide by zero on an empty buffer', () => {
    expect(computeAmplitude(new Float32Array([]))).toBe(0)
  })
})

describe('createVadMessageHandler — forwards mic frames to the main process', () => {
  it('ignores non-audio events but logs them', () => {
    const handler = createVadMessageHandler()
    handler({ data: { type: 'ready' } } as MessageEvent)
    expect(sendVadAudio).not.toHaveBeenCalled()
    expect(rendererLog.debug).toHaveBeenCalledWith('[VAD] Event: ready')
  })

  it('ignores an audio event with no samples', () => {
    const handler = createVadMessageHandler()
    handler({ data: { type: 'audio' } } as MessageEvent)
    expect(sendVadAudio).not.toHaveBeenCalled()
  })

  it('forwards samples with amplitude clamped to [0,1]', () => {
    const handler = createVadMessageHandler()
    const audio = new Float32Array([1, 1, 1, 1])
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    expect(sendVadAudio).toHaveBeenCalledWith({ samples: audio, amplitude: 1, playing: false })
  })

  // The backend only knew when synthesis ended, seconds before playback did: talking over the
  // rest of Mika's answer never interrupted her.
  it('tells the backend whether Mika is still playing audio', () => {
    let playing = true
    const handler = createVadMessageHandler(() => playing)
    const audio = new Float32Array([0.2])
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    expect(sendVadAudio).toHaveBeenLastCalledWith(expect.objectContaining({ playing: true }))

    playing = false
    vi.useFakeTimers().setSystemTime(Date.now() + 50)
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    expect(sendVadAudio).toHaveBeenLastCalledWith(expect.objectContaining({ playing: false }))
  })

  it('throttles emissions to at most one per 10ms window', () => {
    vi.useFakeTimers().setSystemTime(1000)
    const handler = createVadMessageHandler()
    const audio = new Float32Array([0.1, 0.1])
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    vi.setSystemTime(1005)
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    expect(sendVadAudio).toHaveBeenCalledTimes(1)

    vi.setSystemTime(1011)
    handler({ data: { type: 'audio', audio } } as MessageEvent)
    expect(sendVadAudio).toHaveBeenCalledTimes(2)
  })
})

describe('logVadSetupFailure — suppresses expected abort noise', () => {
  it('swallows an AbortError raised while tearing down', () => {
    logVadSetupFailure(new DOMException('aborted', 'AbortError'), true)
    expect(rendererLog.error).not.toHaveBeenCalled()
  })

  it('logs an AbortError that happens while still active — unexpected', () => {
    logVadSetupFailure(new DOMException('aborted', 'AbortError'), false)
    expect(rendererLog.error).toHaveBeenCalled()
  })

  it('logs a non-DOMException failure', () => {
    logVadSetupFailure(new Error('boom'), false)
    expect(rendererLog.error).toHaveBeenCalledWith('[VAD] Setup failed', { detail: 'Error: boom' })
  })
})
