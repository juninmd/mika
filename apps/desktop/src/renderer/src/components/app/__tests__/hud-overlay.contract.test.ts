import { describe, expect, it } from 'vitest'
import { DEFAULT_CAPABILITIES, DEFAULT_SESSION_USAGE } from '../app-shell.types'

// Capability starting `true` lights the vision icon too early; an undefined counter turns into `NaN` in the badge.

describe('DEFAULT_SESSION_USAGE', () => {
  it('todo contador começa em zero — nada de `undefined` virando NaN no badge', () => {
    for (const [campo, valor] of Object.entries(DEFAULT_SESSION_USAGE)) {
      expect(typeof valor, campo).toBe('number')
      expect(Number.isFinite(valor), campo).toBe(true)
      expect(valor, campo).toBe(0)
    }
  })

  it('o custo acumulado começa zerado, e não ausente', () => {
    expect(DEFAULT_SESSION_USAGE.costUsd).toBe(0)
    expect(DEFAULT_SESSION_USAGE.unknownCostTurns).toBe(0)
  })
})

describe('DEFAULT_CAPABILITIES', () => {
  it('nenhuma capacidade nasce ligada — o HUD não promete o que ainda não subiu', () => {
    expect(DEFAULT_CAPABILITIES.hasLLM).toBe(false)
    expect(DEFAULT_CAPABILITIES.canSpeak).toBe(false)
    expect(DEFAULT_CAPABILITIES.canListenAudio).toBe(false)
    expect(DEFAULT_CAPABILITIES.canSeeScreen).toBe(false)
    expect(DEFAULT_CAPABILITIES.canUseTerminal).toBe(false)
    expect(DEFAULT_CAPABILITIES.canLearn).toBe(false)
  })

  it('as contagens começam em zero', () => {
    expect(DEFAULT_CAPABILITIES.plugins).toBe(0)
    expect(DEFAULT_CAPABILITIES.tools).toBe(0)
  })
})
