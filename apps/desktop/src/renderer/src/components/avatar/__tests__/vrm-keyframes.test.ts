import { describe, expect, it } from 'vitest'
import { bell, easePose, sampleKeys } from '../vrm-keyframes'

// New gestures are phrased as keyframes (wind-up, hit, settle) instead of one envelope per pose.
describe('amostragem de keyframes', () => {
  const keys = [
    [0.2, 0],
    [0.4, 1],
    [0.6, 1],
    [0.8, -0.5],
  ] as const

  it('passa exatamente por cada chave', () => {
    for (const [t, v] of keys) expect(sampleKeys(keys, t)).toBeCloseTo(v, 10)
  })

  it('segura o primeiro e o último valor fora do trecho, e trata progresso inválido como início', () => {
    expect(sampleKeys(keys, 0)).toBe(0)
    expect(sampleKeys(keys, 1)).toBe(-0.5)
    expect(sampleKeys(keys, Number.NaN)).toBe(0)
  })

  it('suaviza entre chaves: sai e chega com velocidade nula, sem passar do alvo', () => {
    const near = (t: number) => sampleKeys(keys, t)
    expect(near(0.201)).toBeLessThan(0.001)
    expect(1 - near(0.399)).toBeLessThan(0.001)
    let previous = near(0.2)
    for (let t = 0.2; t <= 0.4; t += 0.005) {
      const value = near(t)
      expect(value).toBeGreaterThanOrEqual(previous - 1e-12)
      expect(value).toBeLessThanOrEqual(1)
      previous = value
    }
  })

  it('chave única vale para todo o intervalo', () => {
    expect(sampleKeys([[0.5, 0.3]], 0.1)).toBe(0.3)
    expect(sampleKeys([[0.5, 0.3]], 0.9)).toBe(0.3)
  })
})

describe('curvas de apoio', () => {
  it('easePose é quíntica com pontas presas em 0 e 1', () => {
    expect(easePose(-1)).toBe(0)
    expect(easePose(0.5)).toBeCloseTo(0.5, 10)
    expect(easePose(2)).toBe(1)
  })

  it('bell nasce e morre em zero e atinge 1 no meio', () => {
    expect(bell(0)).toBe(0)
    expect(bell(1)).toBeCloseTo(0, 10)
    expect(bell(0.5)).toBeCloseTo(1, 10)
    expect(bell(-0.2)).toBe(0)
    expect(bell(1.3)).toBe(0)
  })
})
