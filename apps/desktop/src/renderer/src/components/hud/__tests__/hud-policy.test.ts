import { describe, expect, it } from 'vitest'
import { hudPolicy } from '../hud-policy'

// Transparent is the default mode and sits over games/desktop: whatever the HUD paints at rest
// tints someone else's screen. The policy is the one place that decides what may exist per mode.
describe('hudPolicy', () => {
  it('transparent: zero footprint at rest — no glow, no resting telemetry, dock hides, game collapses to a badge', () => {
    expect(hudPolicy('transparent')).toEqual({
      density: 'minimal',
      dockAutoHide: true,
      dockSurface: 'glass',
      ambientGlow: false,
      telemetryAtRest: false,
      presenceLabelAtRest: false,
      gameHud: 'badge',
    })
  })

  it('semi-transparent: light glass, still auto-hides, game shows as a compact chip', () => {
    const p = hudPolicy('semi-transparent')
    expect(p.density).toBe('light')
    expect(p.dockAutoHide).toBe(true)
    expect(p.ambientGlow).toBe(false)
    expect(p.telemetryAtRest).toBe(false)
    expect(p.gameHud).toBe('chip')
  })

  it('solid: the window owns its pixels — dock stays, telemetry and the game panel show at rest', () => {
    const p = hudPolicy('solid')
    expect(p).toMatchObject({ density: 'full', dockAutoHide: false, telemetryAtRest: true, gameHud: 'panel' })
    expect(p.ambientGlow).toBe(true)
    expect(p.presenceLabelAtRest).toBe(true)
  })

  it('an unknown mode (old config, typo) falls back to the most conservative policy', () => {
    expect(hudPolicy('whatever' as never)).toEqual(hudPolicy('transparent'))
  })
})
