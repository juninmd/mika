import { describe, expect, it } from 'vitest'
import { Live2DAnimator } from '../live2d-animator'
import type { Live2DModelInstance } from '../live2d-loader'

// A real Cubism model loads motion files lazily and keeps earlier motions fading, so a freeze that
// neither stops them nor waits for its own motion photographs whatever the ticker left behind.
function sequencedModel() {
  const events: string[] = []
  const coreModel = {
    _model: { parameters: { ids: ['ParamAngleZ', 'ParamBreath'] } },
    setParameterValueById: () => {},
    getParameterValueById: () => 0,
  }
  const model = {
    internalModel: {
      coreModel,
      lipSyncIds: [],
      settings: {},
      motionManager: { stopAllMotions: () => events.push('stop') },
    },
    update: () => events.push('frame'),
    expression: () => {},
    motion: () => {
      events.push('motion')
      return new Promise<boolean>((resolve) =>
        setTimeout(() => {
          events.push('motion-started')
          resolve(true)
        }, 0),
      )
    },
  } as unknown as Live2DModelInstance
  return { model, events }
}

describe('freeze de uma ação', () => {
  it('para as motions anteriores e só avança quadros depois que a própria motion começou', async () => {
    const { model, events } = sequencedModel()
    const animator = new Live2DAnimator(model, 'haru_greeter_t05')
    animator.start()
    events.length = 0

    expect(await animator.freeze(0.1, 'surprised')).toBe(true)

    expect(events.slice(0, 3)).toEqual(['stop', 'motion', 'motion-started'])
    expect(events.slice(3).every((event) => event === 'frame')).toBe(true)
  })

  it('sem ação ainda limpa motions que sobraram, e não espera nada', async () => {
    const { model, events } = sequencedModel()
    const animator = new Live2DAnimator(model, 'haru_greeter_t05')
    animator.start()
    events.length = 0

    expect(await animator.freeze(0.05)).toBe(true)

    expect(events[0]).toBe('stop')
    expect(events).not.toContain('motion')
  })
})
