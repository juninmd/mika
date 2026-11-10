import { describe, expect, it } from 'vitest'
import { Live2DAnimator } from '../live2d-animator'
import type { Live2DModelInstance } from '../live2d-loader'

// Regression (2026-09-28, stub harness on Haru and Mao): the body rig wrote ParamHairSide/Front/Back
// after `model.update`, and on the next frame Cubism physics — which owns those params in both
// skins' physics3.json — overwrote them before the mesh deformed. Rendered hair was identical with
// wind on and off. Wind has to enter as a physics INPUT: the `wind` option Cubism adds as force to
// every pendulum particle.
function modelWithPhysics() {
  const option = { gravity: { x: 0, y: -1 }, wind: { x: 0, y: 0 } }
  const model = {
    internalModel: {
      coreModel: {
        _model: { parameters: { ids: ['ParamAngleX', 'ParamHairSide', 'ParamBreath'] } },
        setParameterValueById: () => {},
        getParameterValueById: () => 0,
      },
      physics: { getOption: () => option },
      lipSyncIds: [],
      settings: {},
    },
    update: () => {},
    expression: () => {},
    motion: async () => true,
  } as unknown as Live2DModelInstance
  return { model, option }
}

function sampleWindX(strength: number, seconds: number): number[] {
  const { model, option } = modelWithPhysics()
  const animator = new Live2DAnimator(model, 'haru_greeter_t05')
  animator.setWind(strength)
  animator.start()
  const out: number[] = []
  for (let t = 0; t < seconds; t += 1 / 30) {
    animator.update(1 / 30)
    out.push(option.wind.x)
  }
  return out
}

describe('vento no Live2D entra pela física do Cubism', () => {
  it('com vento, a física recebe uma força lateral que varia com as rajadas', () => {
    const xs = sampleWindX(1, 8)
    expect(Math.max(...xs.map(Math.abs))).toBeGreaterThan(0.2)
    expect(new Set(xs.map((x) => x.toFixed(3))).size).toBeGreaterThan(10)
  })

  it('sem vento (desligado ou movimento reduzido), a força é zero', () => {
    for (const x of sampleWindX(0, 2)) expect(x).toBe(0)
  })

  it('a força nunca passa da gravidade: o cabelo balança, não vira de lado', () => {
    for (const x of sampleWindX(1, 30)) expect(Math.abs(x)).toBeLessThanOrEqual(1)
  })

  it('skin sem física não quebra o quadro', () => {
    const { model } = modelWithPhysics()
    ;(model.internalModel as { physics?: unknown }).physics = undefined
    const animator = new Live2DAnimator(model, 'haru_greeter_t05')
    animator.setWind(1)
    animator.start()
    expect(() => animator.update(1 / 30)).not.toThrow()
  })
})
