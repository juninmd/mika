import { describe, expect, it } from 'vitest'
import type { ParameterRange } from '../live2d-animator-parameter-probe'
import { type BodyRigFrame, getPostureForExpression, Live2DBodyRig } from '../live2d-body-rig'

const RIGS: Record<string, ParameterRange[]> = {
  Hiyori: [
    { id: 'ParamArmLB', min: -10, max: 9.58, rest: 0 },
    { id: 'ParamArmRB', min: -10, max: 4.38, rest: 0 },
    { id: 'ParamHandLB', min: -6.25, max: 10, rest: 0 },
    { id: 'ParamHandRB', min: -8.54, max: 10, rest: 0 },
    { id: 'ParamShoulder', min: -1, max: 1, rest: 0 },

    { id: 'ParamHairAhoge', min: -10, max: 10, rest: 0 },
  ],
  Mao: [
    { id: 'ParamArmLA01', min: -6, max: 24, rest: 0 },
    { id: 'ParamArmRA01', min: -4, max: 14, rest: 0 },
    { id: 'ParamArmLA02', min: -20, max: 8, rest: 0 },
    { id: 'ParamArmRA02', min: -20, max: 8, rest: 0 },
    { id: 'ParamArmLA03', min: -18, max: 30, rest: 0 },
    { id: 'ParamArmRA03', min: -18, max: 8, rest: 0 },
    { id: 'ParamLeftShoulderUp', min: -10, max: 10, rest: 0 },
    { id: 'ParamRightShoulderUp', min: -10, max: 8, rest: 0 },
  ],
  Mark: [
    { id: 'ParamArmL', min: -10, max: 30, rest: 0 },
    { id: 'ParamArmR', min: -10, max: 30, rest: 0 },
    { id: 'ParamLeftLeg', min: -3, max: 10, rest: 0 },
    { id: 'ParamRightLeg', min: -3, max: 10, rest: 0 },
  ],
  Rice: [
    { id: 'ParamArmL01', min: -6, max: 7, rest: 0 },
    { id: 'ParamArmR01', min: -21, max: 45, rest: 0 },

    { id: 'ParamArmL03', min: -11, max: 0, rest: 0 },
    { id: 'ParamLegKnee', min: -3, max: 8, rest: 0 },
    { id: 'ParamShoulderL', min: 0, max: 5.2, rest: 0 },
    { id: 'ParamShoulderR', min: 0, max: 10, rest: 0 },
  ],
  haru: [
    { id: 'ParamArmLB', min: -1, max: 5, rest: 0 },

    { id: 'ParamArmRB', min: 0, max: 5, rest: 0 },
    { id: 'ParamHandAngleL', min: -1, max: 1, rest: 0 },
    { id: 'ParamHandAngleR', min: -1, max: 1, rest: 0 },

    { id: 'ParamBodyUpper', min: -7, max: 10, rest: 0 },
  ],
}

function rangesOf(skin: keyof typeof RIGS): Map<string, ParameterRange> {
  return new Map(RIGS[skin].map((range) => [range.id, range]))
}

function frame(overrides: Partial<BodyRigFrame> = {}): BodyRigFrame {
  return {
    dt: 1 / 60,
    time: 0,
    bodySwayVelocity: 0,
    bodyHeaveVelocity: 0,
    headSwayVelocity: 0,
    breath: 0.5,
    posture: getPostureForExpression('neutral'),
    shiver: 0,
    speechEnergy: 0,
    windSide: 0,
    windGust: 0,
    ...overrides,
  }
}

function run(rig: Live2DBodyRig, seconds: number, overrides: Partial<BodyRigFrame> = {}) {
  const written = new Map<string, number[]>()
  const step = 1 / 60
  for (let time = 0; time < seconds; time += step) {
    rig.update(frame({ ...overrides, dt: step, time }), (id, value) => {
      const series = written.get(id) ?? []
      series.push(value)
      written.set(id, series)
    })
  }
  return written
}

describe('Live2DBodyRig — resolução por skin', () => {
  it.each(Object.keys(RIGS))('resolve juntas de corpo na %s', (skin) => {
    const rig = new Live2DBodyRig(rangesOf(skin as keyof typeof RIGS))
    expect(rig.isEmpty).toBe(false)
    expect(rig.listDrivenParams().length).toBeGreaterThan(0)
  })

  it.each(['Hiyori', 'haru'] as const)('não dirige canal de variante de braço na %s', (skin) => {
    const driven = new Live2DBodyRig(rangesOf(skin)).listDrivenParams()
    for (const id of ['ParamArmLA', 'ParamArmRA', 'ParamArmLB', 'ParamArmRB']) {
      expect(driven, `${id} não deveria estar no ocioso`).not.toContain(id)
    }

    expect(driven.length).toBeGreaterThan(0)
  })

  it('dirige o tronco na skin padrão', () => {
    expect(new Live2DBodyRig(rangesOf('haru')).listDrivenParams()).toContain('ParamBodyUpper')
  })

  it.each([
    ['Mao', ['ParamLeftShoulderUp', 'ParamRightShoulderUp']],
    ['Rice', ['ParamShoulderL', 'ParamShoulderR']],
    ['Hiyori', ['ParamShoulder']],
  ] as const)('dirige o ombro da %s', (skin, ids) => {
    const driven = new Live2DBodyRig(rangesOf(skin)).listDrivenParams()
    for (const id of ids) expect(driven).toContain(id)
  })

  it('dirige o ahoge da Hiyori', () => {
    expect(new Live2DBodyRig(rangesOf('Hiyori')).listDrivenParams()).toContain('ParamHairAhoge')
  })

  it('fica vazio e não escreve nada numa skin sem juntas conhecidas', () => {
    const rig = new Live2DBodyRig(new Map([['ParamAngleX', { id: 'ParamAngleX', min: -30, max: 30, rest: 0 }]]))
    expect(rig.isEmpty).toBe(true)
    expect(run(rig, 2).size).toBe(0)
  })

  it('descarta parâmetro com faixa degenerada', () => {
    const rig = new Live2DBodyRig(new Map([['ParamArmL', { id: 'ParamArmL', min: 0, max: 0, rest: 0 }]]))
    expect(rig.isEmpty).toBe(true)
  })
})

describe('Live2DBodyRig — a faixa do rig é respeitada', () => {
  it.each(Object.keys(RIGS))('nunca sai da faixa declarada na %s', (skin) => {
    const ranges = rangesOf(skin as keyof typeof RIGS)
    const rig = new Live2DBodyRig(ranges)

    const written = run(rig, 20, {
      bodySwayVelocity: 400,
      bodyHeaveVelocity: 400,

      headSwayVelocity: 400,
      breath: 1,
      shiver: 1,
      speechEnergy: 1,
      posture: getPostureForExpression('panic'),
    })

    expect(written.size).toBeGreaterThan(0)
    for (const [id, series] of written) {
      const range = ranges.get(id)
      expect(range, `escreveu num parâmetro que não resolveu: ${id}`).toBeDefined()
      for (const value of series) {
        expect(Number.isFinite(value), `${id} virou ${value}`).toBe(true)
        expect(value).toBeGreaterThanOrEqual((range as ParameterRange).min)
        expect(value).toBeLessThanOrEqual((range as ParameterRange).max)
      }
    }
  })

  it('trata faixa assimétrica sem estourar nem travar', () => {
    const rig = new Live2DBodyRig(rangesOf('Rice'))
    const written = run(rig, 15, { bodySwayVelocity: 300, posture: getPostureForExpression('excited') })

    const wrist = written.get('ParamArmL03') ?? []
    expect(wrist.length).toBeGreaterThan(0)
    expect(Math.max(...wrist)).toBeLessThanOrEqual(0)
    expect(Math.min(...wrist)).toBeGreaterThanOrEqual(-11)

    expect(Math.max(...wrist) - Math.min(...wrist)).toBeGreaterThan(1e-3)
  })
})

describe('Live2DBodyRig — movimento de verdade', () => {
  it.each(Object.keys(RIGS))('mexe de fato os braços na %s', (skin) => {
    const rig = new Live2DBodyRig(rangesOf(skin as keyof typeof RIGS))
    const written = run(rig, 30, {
      bodySwayVelocity: 40,
      headSwayVelocity: 40,
      posture: getPostureForExpression('happy'),
    })

    for (const [id, series] of written) {
      const swing = Math.max(...series) - Math.min(...series)
      expect(swing, `${id} ficou parado nos 30s`).toBeGreaterThan(1e-3)
    }
  })

  it('não move os dois braços em espelho', () => {
    const rig = new Live2DBodyRig(rangesOf('Mao'))
    const written = run(rig, 30, { bodySwayVelocity: 20 })

    const left = written.get('ParamArmLA01') ?? []
    const right = written.get('ParamArmRA01') ?? []
    expect(left.length).toBe(right.length)
    expect(left.some((value, index) => Math.abs(value - right[index]) > 1e-3)).toBe(true)
  })

  it('encolhe o movimento em postura cansada', () => {
    const amplitude = (expression: string): number => {
      const rig = new Live2DBodyRig(rangesOf('Mark'))
      const written = run(rig, 25, {
        bodySwayVelocity: 30,
        posture: getPostureForExpression(expression),
      })
      const series = written.get('ParamArmL') ?? []
      return Math.max(...series) - Math.min(...series)
    }

    expect(amplitude('tired')).toBeLessThan(amplitude('excited'))
  })

  it('volta ao repouso quando relaxado', () => {
    const rig = new Live2DBodyRig(rangesOf('Mark'))
    run(rig, 10, { bodySwayVelocity: 200, posture: getPostureForExpression('panic') })

    for (let i = 0; i < 300; i++) rig.relax(1 / 60)

    const written = run(rig, 1 / 30, { bodySwayVelocity: 0 })
    for (const [id, series] of written) {
      for (const value of series) expect(Math.abs(value), `${id} ficou com resíduo`).toBeLessThan(0.2)
    }
  })

  it('é determinístico depois de freeze, mesmo reusando o rig', () => {
    const rig = new Live2DBodyRig(rangesOf('Hiyori'))

    const capture = () => {
      rig.unfreeze()
      run(rig, 3, { bodySwayVelocity: 120, posture: getPostureForExpression('panic') })
      rig.freeze()
      return [...(run(rig, 1, { bodySwayVelocity: 25 }).get('ParamHandLB') ?? [])]
    }

    const first = capture()
    expect(first.length).toBeGreaterThan(0)
    expect(capture()).toEqual(first)
  })
})

// Pedido do usuário (2026-09-28): vento no cabelo também no Live2D, em paridade com as molas do VRM.
describe('Live2DBodyRig — vento no cabelo', () => {
  const HAIR: ParameterRange[] = [
    { id: 'ParamHairFront', min: -1, max: 1, rest: 0 },
    { id: 'ParamHairSide', min: -1, max: 1, rest: 0 },
    { id: 'ParamHairBack', min: -1, max: 1, rest: 0 },
  ]
  const last = (written: Map<string, number[]>, id: string) => written.get(id)?.at(-1) ?? 0

  it('vento de lado empurra as mechas para o mesmo lado, dentro da faixa do parâmetro', () => {
    const ranges = new Map(HAIR.map((r) => [r.id, r]))
    const calm = run(new Live2DBodyRig(ranges), 3)
    const windy = run(new Live2DBodyRig(ranges), 3, { windSide: 0.8, windGust: 0.8 })
    for (const id of ['ParamHairFront', 'ParamHairSide', 'ParamHairBack']) {
      expect(last(windy, id), id).toBeGreaterThan(last(calm, id) + 0.1)
      for (const value of windy.get(id) ?? []) expect(Math.abs(value)).toBeLessThanOrEqual(1)
    }
    const leftward = run(new Live2DBodyRig(ranges), 3, { windSide: -0.8, windGust: 0.8 })
    expect(last(leftward, 'ParamHairSide')).toBeLessThan(-0.1)
  })
})
