import { describe, expect, it } from 'vitest'
import { EFFECT_QUALITY_MAX_FPS, resolveEffectQuality } from '../effect-quality'

describe('resolveEffectQuality', () => {
  const base = { hidden: false, focused: true, onBattery: false }

  it('anima em qualidade cheia na tomada e em foco', () => {
    expect(resolveEffectQuality(base)).toBe('full')
  })

  it('pausa com a janela oculta — não há o que compor', () => {
    expect(resolveEffectQuality({ ...base, hidden: true })).toBe('paused')
  })

  it('sem foco e sem fala entra em eco — a Mika nunca fica travada na tela', () => {
    expect(resolveEffectQuality({ ...base, focused: false })).toBe('eco')
  })

  it('eco também vale na bateria: 15fps custa menos que o reduced de 30', () => {
    expect(resolveEffectQuality({ ...base, focused: false, onBattery: true })).toBe('eco')
    expect(EFFECT_QUALITY_MAX_FPS.eco).toBeLessThan(EFFECT_QUALITY_MAX_FPS.reduced)
  })

  it('continua desenhando sem foco enquanto ela fala', () => {
    expect(resolveEffectQuality({ ...base, focused: false, speaking: true })).toBe('full')
    expect(resolveEffectQuality({ ...base, focused: false, speaking: true, onBattery: true })).toBe('reduced')
  })

  it('oculta vence a fala: janela não composta não desenha nada', () => {
    expect(resolveEffectQuality({ ...base, hidden: true, speaking: true })).toBe('paused')
  })

  it('na bateria reduz, não pausa', () => {
    expect(resolveEffectQuality({ ...base, onBattery: true })).toBe('reduced')
  })

  it('remove movimento contínuo quando o sistema pede movimento reduzido', () => {
    expect(resolveEffectQuality({ ...base, reducedMotion: true })).toBe('paused')
    expect(resolveEffectQuality({ ...base, reducedMotion: true, speaking: true })).toBe('reduced')
  })

  it('bateria não ressuscita o que já estava pausado', () => {
    expect(resolveEffectQuality({ hidden: false, focused: false, onBattery: true, reducedMotion: true })).toBe('paused')
    expect(resolveEffectQuality({ hidden: true, focused: true, onBattery: true })).toBe('paused')
  })

  it('o teto do governador de energia (jogo, ocioso) limita a qualidade, nunca a sobe', () => {
    expect(resolveEffectQuality({ ...base, ceiling: 'reduced' })).toBe('reduced')
    expect(resolveEffectQuality({ ...base, speaking: true, ceiling: 'eco' })).toBe('eco')
    expect(resolveEffectQuality({ ...base, focused: false, ceiling: 'reduced' })).toBe('eco')
    expect(resolveEffectQuality({ ...base, hidden: true, ceiling: 'full' })).toBe('paused')
  })

  it('reduzido custa metade do teto de quadros', () => {
    expect(EFFECT_QUALITY_MAX_FPS.reduced).toBe(EFFECT_QUALITY_MAX_FPS.full / 2)
  })
})
