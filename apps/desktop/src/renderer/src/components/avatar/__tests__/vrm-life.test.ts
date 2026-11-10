import { describe, expect, it } from 'vitest'
import { BLINK_SEC, BlinkDriver } from '../vrm-blink'
import { EYE_INPUT_MAX_DEG, GazeDriver, HEAD_LIMITS, headAnglesToward, tightenEyeRange } from '../vrm-gaze'
import { silentVisemes, VisemeSmoother, visemeTargets } from '../vrm-mouth'
import { blendPresets } from '../vrm-scene-setup'

const run = (driver: BlinkDriver, seconds: number, busy: number) => {
  let peak = 0
  for (let t = 0; t < seconds; t += 1 / 60) peak = Math.max(peak, driver.advance(1 / 60, busy))
  return peak
}

// VRoid's happy/relaxed already close the eye; adding blink on top pushed the eyelid through the cheek.
describe('piscada do VRM', () => {
  it('pisca em olho neutro dentro do intervalo', () => {
    expect(run(new BlinkDriver(() => 0.5), 8, 0)).toBeGreaterThan(0.9)
  })

  it('não pisca enquanto a expressão já fecha o olho', () => {
    expect(run(new BlinkDriver(() => 0.5), 30, 0.8)).toBe(0)
  })

  // Symmetric sin() closed as slowly as it opened; a real lid drops fast and lifts slower.
  it('fecha mais rápido do que abre', () => {
    const driver = new BlinkDriver(() => 0.5)
    const trace: number[] = []
    for (let t = 0; t < 8; t += 1 / 240) trace.push(driver.advance(1 / 240, 0))
    const start = trace.findIndex((v) => v > 0)
    const peak = trace.indexOf(Math.max(...trace))
    const end = trace.findIndex((v, i) => i > peak && v === 0)
    expect(peak - start).toBeLessThan((end - peak) * 0.75)
  })

  it('pisca duas vezes seguidas quando o sorteio pede', () => {
    const driver = new BlinkDriver(() => 0)
    let blinks = 0
    let closed = false
    for (let t = 0; t < 2.8 + BLINK_SEC * 2 + 0.2; t += 1 / 120) {
      const value = driver.advance(1 / 120, 0)
      if (value > 0.5 && !closed) blinks++
      closed = value > 0.5
    }
    expect(blinks).toBe(2)
  })
})

// Raw vowels swapped every frame and made the mouth tremble; release smoothing holds the syllable.
describe('boca do VRM', () => {
  it('fecha mais devagar do que abre', () => {
    const smoother = new VisemeSmoother()
    const open = smoother.advance({ ...silentVisemes(), aa: 1 }, 1 / 60).aa
    const afterOpen = new VisemeSmoother()
    for (let i = 0; i < 60; i++) afterOpen.advance({ ...silentVisemes(), aa: 1 }, 1 / 60)
    const closed = 1 - afterOpen.advance(silentVisemes(), 1 / 60).aa
    expect(open).toBeGreaterThan(closed)
  })

  it('valor inválido do analisador não envenena a boca', () => {
    const smoother = new VisemeSmoother()
    const out = smoother.advance({ ...silentVisemes(), ih: Number.NaN }, 1 / 60)
    expect(Number.isFinite(out.ih)).toBe(true)
  })

  it('em silêncio o ruído não abre a boca além do teto', () => {
    const target = visemeTargets({ a: 1, i: 0, u: 0, e: 0, o: 0 }, 0.9, false)
    expect(target.aa).toBeLessThanOrEqual(0.15)
  })

  it('sem vogal, a abertura vai inteira para "aa"', () => {
    expect(visemeTargets({ a: 0, i: 0, u: 0, e: 0, o: 0 }, 0.6, true)).toEqual({ ...silentVisemes(), aa: 0.6 })
  })
})

describe('olhar do VRM', () => {
  const head = { x: 0, y: 1.4, z: 0 }

  it('a cabeça gira para o lado do alvo e para baixo quando o alvo está abaixo', () => {
    const right = headAnglesToward({ x: 1, y: 1.4, z: 2 }, head)
    const below = headAnglesToward({ x: 0, y: 0.8, z: 2 }, head)
    expect(right.yaw).toBeGreaterThan(0)
    expect(below.pitch).toBeGreaterThan(0)
  })

  it('respeita os limites do pescoço', () => {
    const extreme = headAnglesToward({ x: 50, y: 60, z: 0.1 }, head)
    expect(Math.abs(extreme.yaw)).toBeLessThanOrEqual(HEAD_LIMITS.yaw)
    expect(extreme.pitch).toBeGreaterThanOrEqual(-HEAD_LIMITS.up)
  })

  it('chega ao alvo com amortecimento e as sacadas ficam perto do centro', () => {
    const gaze = new GazeDriver(() => 0.9)
    const desired = { yaw: 0.3, pitch: 0.1 }
    gaze.advance(1 / 60, desired)
    expect(gaze.head.yaw).toBeLessThan(0.3 * 0.2)
    for (let i = 0; i < 300; i++) gaze.advance(1 / 60, desired)
    expect(gaze.head.yaw).toBeCloseTo(0.3, 2)
    expect(Math.abs(gaze.saccade.x)).toBeLessThanOrEqual(0.12)
    expect(gaze.saccade.x).not.toBe(0)
  })

  // Exponential follow jumped to full speed the frame the cursor moved: the head snapped, then eased.
  it('a cabeça acelera e desacelera sem passar do alvo', () => {
    const gaze = new GazeDriver(() => 0.9)
    const desired = { yaw: 0.3, pitch: 0 }
    const steps: number[] = []
    let prev = 0
    let peak = 0
    for (let i = 0; i < 240; i++) {
      gaze.advance(1 / 60, desired)
      steps.push(gaze.head.yaw - prev)
      prev = gaze.head.yaw
      peak = Math.max(peak, gaze.head.yaw)
    }
    expect(steps[1]).toBeGreaterThan(steps[0])
    expect(peak).toBeLessThanOrEqual(0.3 + 1e-6)
    expect(gaze.head.yaw).toBeCloseTo(0.3, 3)
  })

  it('o olho atinge o giro máximo com um olhar menor, sem mudar o giro máximo', () => {
    const applier = {
      rangeMapHorizontalInner: { inputMaxValue: 90, outputScale: 10 },
      rangeMapVerticalUp: { inputMaxValue: 10, outputScale: 10 },
    }
    tightenEyeRange(applier)
    expect(applier.rangeMapHorizontalInner).toEqual({ inputMaxValue: EYE_INPUT_MAX_DEG, outputScale: 10 })
    expect(applier.rangeMapVerticalUp.inputMaxValue).toBe(10)
  })
})

// Exponential blend jumped to its fastest rate on the first frame: the face popped into the new preset.
describe('transição de expressão do VRM', () => {
  it('entra em happy com aceleração suave, sem passar do alvo, em menos de meio segundo', () => {
    const values = new Map<string, number>()
    const em = { getValue: (n: string) => values.get(n) ?? 0, setValue: (n: string, v: number) => values.set(n, v) }
    const velocity: Record<string, number> = {}
    const trace: number[] = []
    for (let i = 0; i < 30; i++) {
      blendPresets(em as unknown as Parameters<typeof blendPresets>[0], 'happy', 1, null, '', 0, 1 / 60, velocity)
      trace.push(em.getValue('happy'))
    }
    expect(trace[1] - trace[0]).toBeGreaterThan(trace[0])
    expect(Math.max(...trace)).toBeLessThanOrEqual(1)
    expect(trace.at(-1)).toBeGreaterThan(0.95)
  })
})
