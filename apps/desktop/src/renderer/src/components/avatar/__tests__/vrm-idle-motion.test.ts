import { describe, expect, it } from 'vitest'
import { IDLE_MOTION_MAX_INTERVAL_SEC } from '../vrm-idle'
import { type IdleMotion, idleMotion } from '../vrm-idle-motion'

const CHANNELS = Object.keys(idleMotion(0)) as (keyof IdleMotion)[]
const DEG = Math.PI / 180

// Between idle poses the body used to move ~1°, which at the window's size reads as a statue. The
// companion must never look frozen: every second of every minute has to carry visible motion.
describe('movimento contínuo do VRM', () => {
  it('nenhum segundo de dez minutos fica parado', () => {
    for (let start = 0; start < 600; start += 0.5) {
      let travel = 0
      for (let t = start; t < start + 1; t += 1 / 30) {
        const a = idleMotion(t)
        const b = idleMotion(t + 1 / 30)
        for (const key of CHANNELS) travel += Math.abs(b[key] - a[key])
      }
      if (travel < 3 * DEG) throw new Error(`parada em t=${start}s: ${(travel / DEG).toFixed(2)}° no segundo`)
    }
  })

  it('o tronco e a cabeça balançam de forma visível, não só a respiração', () => {
    const range = (key: keyof IdleMotion) => {
      const values = Array.from({ length: 1800 }, (_, i) => idleMotion(i / 30)[key])
      return Math.max(...values) - Math.min(...values)
    }
    expect(range('headZ')).toBeGreaterThan(8 * DEG)
    expect(range('headY')).toBeGreaterThan(8 * DEG)
    expect(range('spineZ')).toBeGreaterThan(5 * DEG)
    expect(range('chestX')).toBeGreaterThan(2.5 * DEG)
  })

  it('fica dentro do que o corpo e a saia aguentam', () => {
    const peak = Object.fromEntries(CHANNELS.map((key) => [key, 0])) as unknown as IdleMotion
    for (let t = 0; t < 600; t += 0.1) {
      const m = idleMotion(t)
      for (const key of CHANNELS) peak[key] = Math.max(peak[key], Math.abs(m[key]))
    }
    expect(peak.headZ).toBeLessThan(10 * DEG)
    expect(peak.headY).toBeLessThan(10 * DEG)
    expect(peak.headX).toBeLessThan(5 * DEG)
    expect(peak.spineZ).toBeLessThan(5 * DEG)
    expect(peak.spineY).toBeLessThan(5 * DEG)
    // The pencil skirt clips above ~0.05 rad of hip roll; hips only twist here.
    expect(peak.hipsY).toBeLessThan(2.5 * DEG)
  })

  it('não se repete num ciclo curto que o olho decora', () => {
    const at = (t: number) => CHANNELS.map((key) => idleMotion(t)[key])
    const reference = at(3)
    for (let period = 2; period < 60; period += 0.25) {
      const drift = at(3 + period).reduce((sum, v, i) => sum + Math.abs(v - (reference[i] ?? 0)), 0)
      expect(drift, `repete a cada ${period}s`).toBeGreaterThan(0.5 * DEG)
    }
  })

  it('pose de idle chega em poucos segundos, não em até 14', () => {
    expect(IDLE_MOTION_MAX_INTERVAL_SEC).toBeLessThanOrEqual(8)
  })
})
