import { describe, expect, it } from 'vitest'
import { LIVE2D_ACTION_CATALOG } from '../live2d-action-catalog.generated'
import { Live2DAnimator } from '../live2d-animator'
import type { Live2DModelInstance } from '../live2d-loader'

const HARU = 'haru_greeter_t05'

interface Recorder {
  model: Live2DModelInstance
  written: Map<string, number>
  motions: Array<{ group: string; index: number; priority?: number }>
}

function fakeModel(paramIds: string[]): Recorder {
  const written = new Map<string, number>()
  const motions: Array<{ group: string; index: number; priority?: number }> = []
  const values = new Map<string, number>()

  const coreModel = {
    _model: { parameters: { ids: paramIds } },
    setParameterValueById: (id: string, value: number) => {
      written.set(id, value)
      values.set(id, value)
    },
    getParameterValueById: (id: string) => {
      if (!paramIds.includes(id)) throw new Error(`parâmetro ausente: ${id}`)
      return values.get(id) ?? 0
    },
  }

  const model = {
    internalModel: { coreModel, lipSyncIds: ['ParamMouthOpenY'], settings: {} },
    update: () => {},
    expression: () => {},
    motion: (group: string, index: number, priority?: number) => {
      motions.push({ group, index, priority })
      return Promise.resolve(true)
    },
  } as unknown as Live2DModelInstance

  return { model, written, motions }
}

const HARU_PARAMS = [
  'ParamAngleX',
  'ParamAngleY',
  'ParamAngleZ',
  'ParamBodyAngleX',
  'ParamBodyAngleY',
  'ParamBodyAngleZ',
  'ParamEyeLOpen',
  'ParamEyeROpen',
  'ParamEyeLSmile',
  'ParamEyeRSmile',
  'ParamEyeBallX',
  'ParamEyeBallY',
  'ParamBrowLY',
  'ParamBrowRY',
  'ParamBrowLAngle',
  'ParamBrowRAngle',
  'ParamMouthForm',
  'ParamMouthOpenY',
  'ParamTere',
  'ParamBreath',
]

function startedAnimator(stem: string | undefined = HARU): Recorder & { animator: Live2DAnimator } {
  const recorder = fakeModel(HARU_PARAMS)
  const animator = new Live2DAnimator(recorder.model, stem)
  animator.start()
  return { ...recorder, animator }
}

function advance(animator: Live2DAnimator, seconds: number): void {
  const step = 1 / 60
  for (let elapsed = 0; elapsed < seconds; elapsed += step) animator.update(step)
}

describe('máscara de parâmetros da ação', () => {
  it('cede os canais que a ação dirige enquanto ela toca', () => {
    const { animator, written } = startedAnimator()
    const mask = LIVE2D_ACTION_CATALOG[HARU].nod.params

    expect(animator.playAction('nod')).toBe(true)
    written.clear()
    animator.update(1 / 60)

    for (const param of mask) {
      expect(written.has(param), `o animador escreveu em ${param}, que a ação 'nod' dirige`).toBe(false)
    }

    expect(written.has('ParamBreath')).toBe(true)
  })

  it('retoma a escrita quando a ação termina, contando o fade-out', () => {
    const { animator, written } = startedAnimator()
    const entry = LIVE2D_ACTION_CATALOG[HARU].nod

    animator.playAction('nod')

    advance(animator, entry.duration + entry.fadeOut - 0.2)
    written.clear()
    animator.update(1 / 60)
    expect(written.has('ParamAngleY')).toBe(false)

    advance(animator, 0.4)
    written.clear()
    animator.update(1 / 60)
    expect(written.has('ParamAngleY')).toBe(true)
  })

  it('nunca cede a boca durante a fala — voz ganha de pose', () => {
    const { animator, written } = startedAnimator()
    expect(LIVE2D_ACTION_CATALOG[HARU].happy.params).toContain('ParamMouthOpenY')

    animator.setSpeaking(true)
    animator.playAction('happy')
    written.clear()
    animator.update(1 / 60)

    expect(written.has('ParamMouthOpenY')).toBe(true)

    expect(written.has('ParamAngleY')).toBe(false)
  })
})

describe('playback por nome', () => {
  it('toca no grupo Action, no índice que o catálogo declara', () => {
    const { animator, motions } = startedAnimator()

    expect(animator.playAction('thinking')).toBe(true)
    expect(motions).toEqual([{ group: 'Action', index: LIVE2D_ACTION_CATALOG[HARU].thinking.index, priority: 2 }])
  })

  it('recusa ação que a skin não tem, sem tocar nada', () => {
    const { animator, motions } = startedAnimator('Rice')

    expect(animator.playAction('nod')).toBe(false)
    expect(motions).toEqual([])
  })

  it('skin fora do catálogo fica só no procedural', () => {
    const { animator, written } = startedAnimator('skin-que-nao-existe')

    expect(animator.listActions()).toEqual([])
    expect(animator.playAction('happy')).toBe(false)
    written.clear()
    animator.update(1 / 60)
    expect(written.has('ParamAngleY')).toBe(true)
  })
})

describe('freeze determinístico', () => {
  it('produz a mesma pose para a mesma entrada', async () => {
    const pose = async () => {
      const { animator, written } = startedAnimator()
      await animator.freeze(1.5, 'thinking')
      return [...written.entries()].sort(([a], [b]) => a.localeCompare(b))
    }
    const poses = [await pose(), await pose()]

    expect(poses[0]).toEqual(poses[1])
    expect(poses[0].length).toBeGreaterThan(0)
  })

  it('poses diferentes em instantes diferentes — congelar não é parar no frame 0', async () => {
    const at = async (seconds: number) => {
      const { animator, written } = startedAnimator()
      await animator.freeze(seconds, 'thinking')
      return written.get('ParamBreath')
    }

    expect(await at(0.5)).not.toEqual(await at(2.0))
  })
})

describe('Live2D — o animador cede os canais da física', () => {
  const PHYSICS_OUTPUTS = [
    'ParamHairFront',
    'ParamHairSide',
    'ParamHairBack',
    'ParamScarf',
    'ParamAccessory1',
    'ParamBustY',
  ]

  it('nunca escreve em destino de saída do physics3.json', () => {
    const recorder = fakeModel([...HARU_PARAMS, ...PHYSICS_OUTPUTS])
    const animator = new Live2DAnimator(recorder.model, HARU)
    animator.start()
    animator.setSpeaking(true)
    animator.triggerReactionMotion('panic')
    for (let i = 0; i < 240; i++) animator.update(1 / 60)

    for (const id of PHYSICS_OUTPUTS) {
      expect(recorder.written.has(id), `${id} é dirigido pela física — o animador não pode escrever`).toBe(false)
    }

    expect(recorder.written.has('ParamAngleX')).toBe(true)
  })
})
