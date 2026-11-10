import { describe, expect, it } from 'vitest'
import { DEFAULT_WIND_STRENGTH, SpringWind, type Vec3, WIND_MAX, windInto, windStrengthFromConfig } from '../vrm-wind'

const v = (): Vec3 => ({ x: 0, y: 0, z: 0 })
const length = (w: Vec3) => Math.hypot(w.x, w.y, w.z)
const yaw = (w: Vec3) => Math.atan2(w.x, -w.z)

/** Samples `seconds` of wind at 60 Hz. */
function sweep(seconds: number, strength: number, from = 0): Vec3[] {
  const out: Vec3[] = []
  for (let t = from; t < from + seconds; t += 1 / 60) out.push(windInto(v(), t, strength))
  return out
}

// Pedido do usuário (2026-09-28): "as skins precisam respeitar a física, pode ter ventos que
// impactam no cabelo" — rajadas suaves, direção que muda devagar, força 0..1.
describe('campo de vento', () => {
  it('força zero é ar parado', () => {
    for (const w of sweep(30, 0)) expect(length(w)).toBe(0)
  })

  it('nunca passa do teto proporcional à força', () => {
    for (const strength of [0.3, 1]) {
      for (const w of sweep(600, strength)) expect(length(w)).toBeLessThanOrEqual(strength * WIND_MAX + 1e-9)
    }
  })

  it('é baixa frequência: de um quadro para o outro o vento quase não muda', () => {
    const samples = sweep(300, 1)
    for (let i = 1; i < samples.length; i++) {
      const a = samples[i - 1]
      const b = samples[i]
      expect(Math.hypot(b.x - a.x, b.y - a.y, b.z - a.z)).toBeLessThan(WIND_MAX * 0.02)
    }
  })

  it('tem rajadas e calmarias, não um sopro constante', () => {
    const lengths = sweep(600, 1).map(length)
    expect(Math.max(...lengths)).toBeGreaterThan(Math.min(...lengths) * 2)
  })

  it('a direção passeia ao longo dos minutos, mas não vira de um segundo para o outro', () => {
    const yaws = sweep(600, 1).map(yaw)
    expect(Math.max(...yaws) - Math.min(...yaws)).toBeGreaterThan(0.5)
    for (let i = 60; i < yaws.length; i += 60) expect(Math.abs(yaws[i] - yaws[i - 60])).toBeLessThan(0.12)
  })

  it('a força escala o vento linearmente, sem mudar o desenho das rajadas', () => {
    const half = windInto(v(), 123.4, 0.4)
    const full = windInto(v(), 123.4, 0.8)
    expect(full.x).toBeCloseTo(half.x * 2, 9)
    expect(full.z).toBeCloseTo(half.z * 2, 9)
  })

  it('não aloca: escreve no objeto dado, e força inválida vira ar parado', () => {
    const out = v()
    expect(windInto(out, 5, 0.5)).toBe(out)
    expect(length(windInto(out, 5, Number.NaN))).toBe(0)
  })
})

interface FakeBone {
  parent: FakeBone | null
}

function joint(bone: FakeBone, power = 0.02) {
  const dir = {
    x: 0,
    y: -1,
    z: 0,
    set(x: number, y: number, z: number) {
      this.x = x
      this.y = y
      this.z = z
    },
  }
  return { bone, settings: { gravityPower: power, gravityDir: dir } }
}

/** A chain of `n` spring joints hanging off `root`. */
function chain(n: number, root: FakeBone = { parent: null }) {
  const joints = []
  let parent = root
  for (let i = 0; i < n; i++) {
    const bone = { parent }
    joints.push(joint(bone))
    parent = bone
  }
  return joints
}

describe('vento nas molas', () => {
  it('mecha longa balança mais que fita curta', () => {
    const hair = chain(6)
    const ribbon = chain(2)
    const wind = new SpringWind([...hair, ...ribbon])
    wind.apply({ x: 0.05, y: 0, z: 0 })
    const sideways = (j: ReturnType<typeof joint>) => j.settings.gravityDir.x * j.settings.gravityPower
    expect(sideways(hair[5])).toBeGreaterThan(sideways(ribbon[1]) * 1.5)
    // Within a chain, the tip catches more wind than the root.
    expect(sideways(hair[5])).toBeGreaterThan(sideways(hair[0]))
  })

  it('sem vento a gravidade autoral fica intacta, e restore devolve o que o modelo trouxe', () => {
    const joints = chain(3)
    const wind = new SpringWind(joints)
    wind.apply({ x: 0, y: 0, z: 0 })
    expect(joints[2].settings.gravityPower).toBeCloseTo(0.02, 9)
    expect(joints[2].settings.gravityDir.y).toBeCloseTo(-1, 9)
    wind.apply({ x: 0.08, y: 0, z: -0.05 })
    wind.restore()
    for (const j of joints) {
      expect(j.settings.gravityPower).toBe(0.02)
      expect(j.settings.gravityDir.y).toBe(-1)
    }
  })
})

describe('força do vento a partir da config', () => {
  it('config ausente usa o padrão do schema; desligado ou movimento reduzido é ar parado', () => {
    expect(windStrengthFromConfig(undefined, undefined, false)).toBe(DEFAULT_WIND_STRENGTH)
    expect(windStrengthFromConfig(true, 80, false)).toBeCloseTo(0.8, 9)
    expect(windStrengthFromConfig(false, 80, false)).toBe(0)
    expect(windStrengthFromConfig(true, 80, true)).toBe(0)
  })

  it('valor fora da faixa ou lixo não vira furacão', () => {
    expect(windStrengthFromConfig(true, 250, false)).toBe(1)
    expect(windStrengthFromConfig(true, -5, false)).toBe(0)
    expect(windStrengthFromConfig(true, 'forte', false)).toBe(DEFAULT_WIND_STRENGTH)
  })
})
