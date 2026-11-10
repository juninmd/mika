import { describe, expect, it } from 'vitest'
import { ttsLevelWrite } from '../tts-level-write'

describe('ttsLevelWrite — quantização a 3 casas', () => {
  it('suaviza dentro do mesmo degrau: devolve o último valor escrito, sem mudança', () => {
    // 0.4254 e 0.4256 caem no mesmo degrau 0.425 → sem escrita nova.
    expect(ttsLevelWrite(0.4254, 0.425)).toBe(0.425)
    expect(ttsLevelWrite(0, 0)).toBe(0)
  })

  it('mudança mínima visível: devolve o degrau quantizado', () => {
    expect(ttsLevelWrite(0.426, 0.425)).toBe(0.426)
    expect(ttsLevelWrite(0.2517, 0)).toBe(0.252)
  })

  it('satura a faixa 0..1 antes de quantizar', () => {
    expect(ttsLevelWrite(1.6, 0.9)).toBe(1)
    expect(ttsLevelWrite(-0.3, 0)).toBe(0)
  })

  it('primeira escrita após o cleanup (-1) sempre sai', () => {
    expect(ttsLevelWrite(0, -1)).toBe(0)
    expect(ttsLevelWrite(0.5, -1)).toBe(0.5)
  })
})
