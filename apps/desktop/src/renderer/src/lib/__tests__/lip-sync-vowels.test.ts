import { describe, expect, it } from 'vitest'
import {
  dominantVowel,
  estimateFormants,
  loudnessToOpenness,
  rmsLoudness,
  SILENT_VOWELS,
  vowelWeights,
} from '../lip-sync-vowels'

const SAMPLE_RATE = 48_000
const FFT_SIZE = 2048
const BIN_SIZE = SAMPLE_RATE / FFT_SIZE

function spectrumWithFormants(f1: number, f2: number, level = 200): Uint8Array {
  const data = new Uint8Array(FFT_SIZE / 2)
  data.fill(4)
  data[Math.round(f1 / BIN_SIZE)] = level
  data[Math.round(f2 / BIN_SIZE)] = level
  return data
}

function sine(amplitude: number, samples = 1024): Float32Array {
  const out = new Float32Array(samples)
  for (let i = 0; i < samples; i++) out[i] = Math.sin((i / samples) * Math.PI * 2 * 8) * amplitude
  return out
}

describe('rmsLoudness', () => {
  it('devolve 0 no silêncio absoluto', () => {
    expect(rmsLoudness(new Float32Array(512))).toBe(0)
  })

  it('cresce junto com a amplitude', () => {
    expect(rmsLoudness(sine(0.5))).toBeGreaterThan(rmsLoudness(sine(0.1)))
  })

  it('não depende do tamanho do quadro', () => {
    const curto = rmsLoudness(sine(0.4, 256))
    const longo = rmsLoudness(sine(0.4, 4096))
    expect(Math.abs(curto - longo)).toBeLessThan(0.02)
  })
})

describe('loudnessToOpenness', () => {
  it('dá valores distintos ao longo da faixa de fala, sem saturar', () => {
    const níveis = [0.01, 0.02, 0.04, 0.08, 0.15, 0.25]
    const aberturas = níveis.map(loudnessToOpenness)

    expect(new Set(aberturas.map((v) => v.toFixed(3))).size).toBe(níveis.length)
    for (const abertura of aberturas) expect(abertura).toBeLessThan(1)

    for (let i = 1; i < aberturas.length; i++) expect(aberturas[i]).toBeGreaterThan(aberturas[i - 1] as number)
  })

  it('fecha a boca no silêncio e no ruído de sala', () => {
    expect(loudnessToOpenness(0)).toBe(0)
    expect(loudnessToOpenness(0.0001)).toBe(0)
  })

  it('satura só em sinal muito alto', () => {
    expect(loudnessToOpenness(1)).toBe(1)
  })
})

describe('estimateFormants', () => {
  it('acha os dois picos dentro das faixas de F1 e F2', () => {
    const { f1, f2 } = estimateFormants(spectrumWithFormants(750, 1300), SAMPLE_RATE, FFT_SIZE)
    expect(Math.abs(f1 - 750)).toBeLessThan(BIN_SIZE * 1.5)
    expect(Math.abs(f2 - 1300)).toBeLessThan(BIN_SIZE * 1.5)
  })

  it('devolve zero quando só há ruído de fundo', () => {
    const ruído = new Uint8Array(FFT_SIZE / 2)
    ruído.fill(3)
    const { f1, f2 } = estimateFormants(ruído, SAMPLE_RATE, FFT_SIZE)
    expect(f1).toBe(0)
    expect(f2).toBe(0)
  })
})

describe('vowelWeights', () => {
  const casos: Array<[string, number, number]> = [
    ['a', 750, 1300],
    ['i', 300, 2400],
    ['u', 350, 800],
    ['e', 500, 1900],
    ['o', 500, 950],
  ]

  it.each(casos)('classifica /%s/ pelos seus formantes', (esperada, f1, f2) => {
    const pesos = vowelWeights({ f1, f2 }, 0.7)
    expect(dominantVowel(pesos).vowel).toBe(esperada)
  })

  it('soma 1 quando há fala', () => {
    const pesos = vowelWeights({ f1: 600, f2: 1500 }, 0.5)
    const soma = pesos.a + pesos.i + pesos.u + pesos.e + pesos.o
    expect(soma).toBeCloseTo(1, 5)
  })

  it('mistura vogais vizinhas em vez de cravar uma só', () => {
    const pesos = vowelWeights({ f1: 420, f2: 870 }, 0.6)
    expect(dominantVowel(pesos).weight).toBeLessThan(0.8)
    expect(pesos.o).toBeGreaterThan(0.1)
    expect(pesos.u).toBeGreaterThan(0.1)
  })

  it('zera tudo no silêncio — vogal sem voz é sorteio', () => {
    expect(vowelWeights({ f1: 750, f2: 1300 }, 0.01)).toEqual(SILENT_VOWELS)
  })

  it('zera tudo quando não achou formante', () => {
    expect(vowelWeights({ f1: 0, f2: 0 }, 0.9)).toEqual(SILENT_VOWELS)
  })
})
