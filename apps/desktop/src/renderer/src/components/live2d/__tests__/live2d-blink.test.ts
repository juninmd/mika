import { describe, expect, it } from 'vitest'
import { updateBlink } from '../live2d-animator-blink'
import { type AnimatorState, createAnimatorState } from '../live2d-animator-state'

// Same rule as the VRM blink (vrm-life.test.ts): the lid drops fast and lifts slower. Close and open
// used the same speed, so every blink read as a mechanical shutter.
describe('piscada do Live2D', () => {
  it('fecha mais rápido do que abre', () => {
    const s: AnimatorState = { ...createAnimatorState(), blinkState: 'closing' as AnimatorState['blinkState'] }
    const dt = 1 / 240
    let closing = 0
    let opening = 0
    for (let i = 0; i < 480 && !(s.blinkState === 'open' && opening > 0); i++) {
      if (s.blinkState === 'closing') closing += dt
      if (s.blinkState === 'opening') opening += dt
      updateBlink(s, dt)
    }
    expect(closing).toBeLessThan(opening * 0.75)
  })
})
