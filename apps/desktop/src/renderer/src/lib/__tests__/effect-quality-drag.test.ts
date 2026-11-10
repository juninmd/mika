import { afterEach, describe, expect, it } from 'vitest'
import { resolveEffectQuality, setWindowDragging } from '../effect-quality'

const focused = { hidden: false, focused: true, onBattery: false }

describe('qualidade durante o arrasto nativo da janela', () => {
  afterEach(() => setWindowDragging(false))

  it('limita a 30 fps enquanto arrasta e volta a 60 ao soltar', () => {
    expect(resolveEffectQuality(focused)).toBe('full')
    setWindowDragging(true)
    expect(resolveEffectQuality(focused)).toBe('reduced')
    expect(resolveEffectQuality({ ...focused, ceiling: 'eco' })).toBe('eco')
    expect(resolveEffectQuality({ ...focused, hidden: true })).toBe('paused')
    setWindowDragging(false)
    expect(resolveEffectQuality(focused)).toBe('full')
  })
})
