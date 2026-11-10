import { beforeEach, describe, expect, it, vi } from 'vitest'
import { LipSyncSource } from '../lip-sync-source'

type FakeAnalyser = {
  fftSize: number
  frequencyBinCount: number
  smoothingTimeConstant: number
  getByteFrequencyData: ReturnType<typeof vi.fn>
  getFloatTimeDomainData: ReturnType<typeof vi.fn>
  connect: ReturnType<typeof vi.fn>
}

function makeCtx(analyser: FakeAnalyser) {
  const mediaSourceNode = { connect: vi.fn() }
  const createMediaElementSource = vi.fn(() => mediaSourceNode)
  const gainNode = { connect: vi.fn(), gain: { value: 1 } }
  const createGain = vi.fn(() => gainNode)
  const ctx = { sampleRate: 48000, createMediaElementSource, createGain } as unknown as AudioContext
  return { ctx, analyser: analyser as unknown as AnalyserNode, mediaSourceNode, gainNode }
}

function makeAnalyser(): FakeAnalyser {
  return {
    fftSize: 1024,
    frequencyBinCount: 512,
    smoothingTimeConstant: 0.55,
    getByteFrequencyData: vi.fn(),

    getFloatTimeDomainData: vi.fn(),
    connect: vi.fn(),
  }
}

describe('LipSyncSource', () => {
  beforeEach(() => {
    vi.restoreAllMocks()

    Object.defineProperty(globalThis, 'requestAnimationFrame', { value: () => 1, configurable: true })
    Object.defineProperty(globalThis, 'cancelAnimationFrame', { value: () => void 0, configurable: true })
  })

  it('roteia o fallback <audio> pelo analisador (o elo que faltava)', () => {
    const { ctx, analyser, mediaSourceNode } = makeCtx(makeAnalyser())
    const source = new LipSyncSource({ ctx, analyser, volumeRef: { current: 0 } })
    const el = {} as HTMLMediaElement

    source.attachMediaElement(el)
    expect(ctx.createMediaElementSource).toHaveBeenCalledWith(el)
    expect(mediaSourceNode.connect).toHaveBeenCalledWith(analyser)

    source.attachMediaElement(el)
    expect(ctx.createMediaElementSource).toHaveBeenCalledTimes(1)
  })

  it('liga a fonte decodificada ao analisador via GainNode (fade-out do barge-in)', () => {
    const { ctx, analyser, gainNode } = makeCtx(makeAnalyser())
    const source = new LipSyncSource({ ctx, analyser, volumeRef: { current: 0 } })
    const bufferSource = { connect: vi.fn() } as unknown as AudioBufferSourceNode

    const returnedGain = source.attachBufferSource(bufferSource)
    expect(bufferSource.connect).toHaveBeenCalledWith(gainNode)
    expect(gainNode.connect).toHaveBeenCalledWith(analyser)
    expect(returnedGain).toBe(gainNode)
  })

  it('zera o sinal ao parar', () => {
    const { ctx, analyser } = makeCtx(makeAnalyser())
    const volumeRef = { current: 0.7 }
    const source = new LipSyncSource({ ctx, analyser, volumeRef })

    source.setPlaying(true)
    source.setPlaying(false)
    expect(volumeRef.current).toBe(0)
  })

  it('degrada o envelope a zero sem limites de palavra', () => {
    const { ctx, analyser } = makeCtx(makeAnalyser())
    const source = new LipSyncSource({ ctx, analyser, volumeRef: { current: 0 } })

    expect(() => source.setWordBoundaries([])).not.toThrow()
  })
})
