import { describe, expect, it } from 'vitest'
import { frameSubsteps, lerp, MAX_FRAME_DELTA_SECONDS, Spring } from '../live2d-physics'

const advance = (spring: Spring, rawDt: number) => {
  const { steps, step } = frameSubsteps(rawDt)
  for (let i = 0; i < steps; i++) spring.update(step)
}

describe('estabilidade da animação com passos limitados', () => {
  it('sem o limite, o lerp ultrapassa o alvo', () => {
    expect(lerp(0, 1, 12 * 10)).toBeGreaterThan(1)
  })

  it('com o limite, o lerp de um passo nunca passa do alvo', () => {
    expect(lerp(0, 1, Math.min(frameSubsteps(12).step * 10, 1))).toBeLessThanOrEqual(1)
  })

  it('a mola converge para o alvo em passos limitados, sem divergir', () => {
    const spring = new Spring(0, 80, 6)
    spring.target = 1
    for (let frame = 0; frame < 600; frame++) advance(spring, 1 / 60)
    expect(spring.current).toBeCloseTo(1, 2)
  })

  it('a mola sobrevive a uma pausa longa da janela', () => {
    const spring = new Spring(0, 80, 6)
    spring.target = 1
    advance(spring, 45)
    for (let frame = 0; frame < 300; frame++) advance(spring, 1 / 60)
    expect(Number.isFinite(spring.current)).toBe(true)
    expect(Math.abs(spring.current)).toBeLessThan(10)
  })

  it('sem o limite, a mesma pausa faz a mola divergir', () => {
    const spring = new Spring(0, 80, 6)
    spring.target = 1
    spring.update(45)
    expect(Math.abs(spring.current)).toBeGreaterThan(100)
  })
})

// Clamping a 15 fps (eco) frame to 1/30 s ran breathing, blinks and springs at half speed whenever
// the companion sat unfocused; substeps keep real time with each step still stable.
describe('frameSubsteps', () => {
  it('a 15 fps o animador anda o tempo real em passos estáveis', () => {
    const { steps, step } = frameSubsteps(1 / 15)
    expect(steps * step).toBeCloseTo(1 / 15, 6)
    expect(step).toBeLessThanOrEqual(MAX_FRAME_DELTA_SECONDS)
  })

  it('quadro normal de 60 fps é um passo só', () => {
    expect(frameSubsteps(1 / 60)).toEqual({ steps: 1, step: 1 / 60 })
  })

  it('uma pausa longa vira um alcance curto, não segundos de simulação', () => {
    const { steps, step } = frameSubsteps(12)
    expect(steps * step).toBeLessThanOrEqual(0.25)
  })

  it.each([Number.NaN, -1, 0])('delta inválido %p não simula nada', (value) => {
    expect(frameSubsteps(value).steps).toBe(0)
  })
})
