import { describe, expect, it } from 'vitest'
import { BlinkDriver } from '../vrm-blink'
import { ARM_REST, BODY_ACTION_SECONDS, VRM_BODY_ACTIONS, VrmArmDriver } from '../vrm-body-rig'
import { GazeDriver } from '../vrm-gaze'
import { hop, swell } from '../vrm-keyframes'

const FRAME = 1 / 60

describe('movimento natural dos gestos', () => {
  it('hop tem a faixa e o período de |sin| mas sem quina: derivada contínua nos zeros', () => {
    expect(hop(Math.PI / 2)).toBeCloseTo(1, 6)
    expect(hop(Math.PI)).toBeCloseTo(0, 6)
    const slope = (x: number) => (hop(x + 1e-4) - hop(x - 1e-4)) / 2e-4
    expect(Math.abs(slope(Math.PI))).toBeLessThan(1e-3)
  })

  it('swell entra parado e sai parado', () => {
    expect(swell(0.3)).toBe(0)
    expect(swell(0.44)).toBe(1)
    const slope = (p: number) => (swell(p + 1e-5) - swell(p - 1e-5)) / 2e-5
    expect(Math.abs(slope(0.3001))).toBeLessThan(0.01)
  })

  // Os gestos rítmicos tinham quinas de |sin| (~450 rad/s²) e balanços que nasciam na velocidade máxima:
  // o braço "arrancava" e o joelho "batia". A régua é pico de aceleração por junta, em quadros de 60 Hz.
  it.each(VRM_BODY_ACTIONS)('%s não tem solavanco de aceleração em nenhuma junta', (action) => {
    const driver = new VrmArmDriver()
    driver.play(action)
    const keys = Object.keys(ARM_REST) as (keyof typeof ARM_REST)[]
    const joints = keys.filter((key) => !/Grip|Peace|Thumb|heart/.test(key))
    let prev: Record<string, number> = { ...ARM_REST }
    const prevVelocity: Record<string, number> = {}
    let peak = 0
    for (let i = 0; i < Math.ceil(BODY_ACTION_SECONDS[action] / FRAME) + 3; i++) {
      const pose = driver.advance(FRAME) ?? ARM_REST
      for (const key of joints) {
        const velocity = (pose[key] - prev[key]) / FRAME
        if (i > 1) peak = Math.max(peak, Math.abs((velocity - (prevVelocity[key] ?? 0)) / FRAME))
        prevVelocity[key] = velocity
      }
      prev = { ...pose }
    }
    expect(peak).toBeLessThan(130)
  })
})

describe('piscada acompanha o olhar', () => {
  const calm = () => 0.5

  it('um giro rápido da cabeça antecipa a piscada que já estava próxima', () => {
    const blink = new BlinkDriver(calm)
    for (let t = 0; t < 1.5; t += FRAME) blink.advance(FRAME, 0, 0)
    let closed = 0
    for (let i = 0; i < 6; i++) closed = Math.max(closed, blink.advance(FRAME, 0, 1.2))
    expect(closed).toBeGreaterThan(0.3)
  })

  it('olhar parado não antecipa nada', () => {
    const blink = new BlinkDriver(calm)
    for (let t = 0; t < 1.5; t += FRAME) blink.advance(FRAME, 0, 0)
    let closed = 0
    for (let i = 0; i < 6; i++) closed = Math.max(closed, blink.advance(FRAME, 0, 0))
    expect(closed).toBe(0)
  })

  it('não repete a piscada dentro de 1 s da anterior, mesmo com a cabeça girando', () => {
    const blink = new BlinkDriver(calm)
    for (let t = 0; t < 1.5; t += FRAME) blink.advance(FRAME, 0, 1.2)
    // depois da primeira piscada antecipada o gatilho fica em recarga
    let blinks = 0
    let was = 0
    for (let t = 0; t < 1; t += FRAME) {
      const v = blink.advance(FRAME, 0, 1.2)
      if (v > 0.5 && was <= 0.5) blinks++
      was = v
    }
    expect(blinks).toBeLessThanOrEqual(1)
  })

  it('expressão que já fecha o olho segura a piscada', () => {
    const blink = new BlinkDriver(calm)
    for (let t = 0; t < 1.5; t += FRAME) blink.advance(FRAME, 1, 0)
    expect(blink.advance(FRAME, 1, 2)).toBe(0)
  })

  it('falando, pisca com mais frequência que em silêncio (conversa ~1,5x a taxa de repouso)', () => {
    const countBlinks = (speaking: boolean) => {
      const blink = new BlinkDriver(calm)
      let blinks = 0
      let was = 0
      for (let t = 0; t < 120; t += FRAME) {
        const v = blink.advance(FRAME, 0, 0, speaking)
        if (v > 0.5 && was <= 0.5) blinks++
        was = v
      }
      return blinks
    }
    expect(countBlinks(true)).toBeGreaterThan(countBlinks(false) * 1.3)
  })

  it('começar a falar encurta uma espera longa já agendada', () => {
    const blink = new BlinkDriver(() => 0.99)
    for (let t = 0; t < 0.5; t += FRAME) blink.advance(FRAME, 0, 0, false)
    let closed = 0
    for (let t = 0; t < 4.5; t += FRAME) closed = Math.max(closed, blink.advance(FRAME, 0, 0, true))
    expect(closed).toBeGreaterThan(0.5)
  })

  it('o GazeDriver informa a velocidade angular da cabeça', () => {
    const gaze = new GazeDriver(calm)
    expect(gaze.speed).toBe(0)
    gaze.advance(FRAME, { yaw: 0.6, pitch: 0 })
    expect(gaze.speed).toBeGreaterThan(0)
    for (let i = 0; i < 600; i++) gaze.advance(FRAME, { yaw: 0.6, pitch: 0 })
    expect(gaze.speed).toBeLessThan(0.01)
  })
})
