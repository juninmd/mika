import { describe, expect, it } from 'vitest'
import {
  dominantVowel,
  estimateFormants,
  loudnessToOpenness,
  rmsLoudness,
  SILENT_VOWELS,
  vowelWeights,
} from '../lip-sync-vowels'
import { getFriendlyToolLabel } from '../tool-labels'

describe('tool-labels', () => {
  it('retorna labels amigáveis para ferramentas conhecidas', () => {
    expect(getFriendlyToolLabel('vision_capture')).toBe('Olhando a tela...')
    expect(getFriendlyToolLabel('steam')).toBe('Consultando Steam...')
    expect(getFriendlyToolLabel('clear_history')).toBe('Apagando histórico...')
  })

  it('retorna fallback formatado para ferramentas desconhecidas', () => {
    expect(getFriendlyToolLabel('custom_unknown_tool')).toBe('custom_unknown_tool')
  })
})

describe('lip-sync-vowels', () => {
  it('calcula RMS loudness para silêncio e amostras de áudio', () => {
    expect(rmsLoudness(new Float32Array([]))).toBe(0)
    expect(rmsLoudness(new Float32Array([0, 0, 0, 0]))).toBe(0)

    const samples = new Float32Array([1, -1, 1, -1])
    expect(rmsLoudness(samples)).toBe(1)
  })

  it('converte loudness para abertura da boca (0 a 1)', () => {
    expect(loudnessToOpenness(0)).toBe(0)
    // Abaixo do threshold de silêncio (-55 dBFS)
    expect(loudnessToOpenness(0.0001)).toBe(0)
    // No pico alto (-12 dBFS ou acima)
    expect(loudnessToOpenness(1.0)).toBe(1)
  })

  it('estima formantes a partir de FFT', () => {
    const fftSize = 1024
    const sampleRate = 44100
    const freqData = new Uint8Array(fftSize / 2).fill(0)

    // Silêncio
    const silent = estimateFormants(freqData, sampleRate, fftSize)
    expect(silent.f1).toBe(0)
    expect(silent.f2).toBe(0)

    // Com pico forte no range do F1 (~750Hz)
    const binSize = sampleRate / fftSize // ~43.06 Hz
    const targetBin = Math.round(750 / binSize)
    freqData[targetBin] = 200

    const estimated = estimateFormants(freqData, sampleRate, fftSize)
    expect(estimated.f1).toBeGreaterThan(0)
  })

  it('retorna vogais silenciosas quando loudness é baixo', () => {
    const weights = vowelWeights({ f1: 750, f2: 1300 }, 0.05)
    expect(weights).toEqual(SILENT_VOWELS)
  })

  it('classifica a vogal dominante corretamente', () => {
    const dominant = dominantVowel({ a: 0.8, i: 0.1, u: 0.05, e: 0.03, o: 0.02 })
    expect(dominant.vowel).toBe('a')
    expect(dominant.weight).toBe(0.8)

    const dominantI = dominantVowel({ a: 0.1, i: 0.9, u: 0, e: 0, o: 0 })
    expect(dominantI.vowel).toBe('i')
    expect(dominantI.weight).toBe(0.9)
  })
})
