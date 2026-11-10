import { describe, expect, it } from 'vitest'
import {
  bladePoseInto,
  PETAL_CAPACITY,
  PETAL_LIFE_SEC,
  PetalSwarm,
  REDUCED_PETALS,
  SENBONZAKURA_PETALS,
  SENBONZAKURA_TIMELINE as T,
} from '../vrm-senbonzakura'

const pose = (progress: number) => bladePoseInto({ hilt: 0, raise: 0, join: 0, flip: 0, bladeVisible: false }, progress)

// "Chire, Senbonzakura": draw, raise before the face, hold, turn the blade down, let go — petals.
describe('linha do tempo da senbonzakura', () => {
  it('as fases vêm na ordem certa e cabem na ação', () => {
    expect(T.drawAt).toBeLessThan(T.raisedAt)
    expect(T.raisedAt).toBeLessThan(T.flipFrom)
    expect(T.flipFrom).toBeLessThan(T.flipTo)
    expect(T.flipTo).toBeLessThanOrEqual(T.scatterAt)
    expect(T.scatterAt).toBeLessThan(T.hiltGoneAt)
    expect(T.hiltGoneAt).toBeLessThan(1)
  })

  it('sem espada antes de sacar e depois do fim', () => {
    expect(pose(0).hilt).toBe(0)
    expect(pose(1).hilt).toBe(0)
    expect(pose(Number.NaN).hilt).toBe(0)
  })

  it('ergue a lâmina até a pausa, segura, e só vira para baixo na soltura', () => {
    const held = pose((T.raisedAt + T.flipFrom) / 2)
    expect(held.raise).toBeCloseTo(1, 5)
    expect(held.flip).toBe(0)
    expect(held.hilt).toBeCloseTo(1, 5)
    expect(held.bladeVisible).toBe(true)
    expect(pose(T.flipTo).flip).toBeCloseTo(1, 5)
  })

  // Pedido do usuário (2026-09-28): "ela precisa segurar melhor a espada" — guarda a duas mãos.
  it('a segunda mão fecha no cabo durante a guarda e larga antes de virar a lâmina', () => {
    expect(pose(T.drawAt).join).toBe(0)
    expect(pose(T.raisedAt).join).toBeCloseTo(1, 5)
    expect(pose(T.flipFrom).join).toBeCloseTo(1, 5)
    expect(pose(T.flipTo).join).toBe(0)
  })

  // Pedido do usuário (2026-09-28): o cabo ficava na mão até 90% da jogada, bem depois das pétalas;
  // a katana inteira tem de se desfazer junto com a soltura.
  it('a lâmina some na soltura e o cabo se desfaz junto, em menos de meio segundo', () => {
    expect(pose(T.scatterAt - 0.001).bladeVisible).toBe(true)
    expect(pose(T.scatterAt - 0.001).hilt).toBeCloseTo(1, 5)
    expect(pose(T.scatterAt).bladeVisible).toBe(false)
    expect(pose(T.hiltGoneAt).hilt).toBeLessThan(0.01)
    expect(T.hiltGoneAt - T.scatterAt).toBeLessThan(0.081)
  })

  it('reusa o objeto de saída', () => {
    const out = { hilt: 0, raise: 0, join: 0, flip: 0, bladeVisible: false }
    expect(bladePoseInto(out, 0.3)).toBe(out)
  })
})

function spawned(count = SENBONZAKURA_PETALS, rand: () => number = Math.random) {
  const swarm = new PetalSwarm()
  swarm.spawn(
    count,
    (i, out) => {
      out.x = 0.2
      out.y = 1 + (i / count) * 0.7
      out.z = 0.3
    },
    { x: 0, z: 0 },
    rand,
  )
  return swarm
}

const radiusOf = (s: PetalSwarm, i: number) => Math.hypot(s.x[i], s.z[i])
const meanRadius = (s: PetalSwarm) => {
  let sum = 0
  for (let i = 0; i < s.count; i++) sum += radiusOf(s, i)
  return sum / s.count
}
const run = (s: PetalSwarm, seconds: number) => {
  for (let t = 0; t < seconds; t += 1 / 60) s.step(1 / 60)
}

describe('enxame de pétalas', () => {
  it('mil pétalas numa única frota, limitada à capacidade do pool', () => {
    expect(SENBONZAKURA_PETALS).toBeGreaterThanOrEqual(800)
    expect(SENBONZAKURA_PETALS).toBeLessThanOrEqual(PETAL_CAPACITY)
    expect(spawned().count).toBe(SENBONZAKURA_PETALS)
    expect(spawned(PETAL_CAPACITY * 3).count).toBe(PETAL_CAPACITY)
    expect(REDUCED_PETALS).toBeLessThan(200)
  })

  it('nascem na lâmina e passam a orbitar em volta dela', () => {
    const swarm = spawned()
    swarm.step(0)
    expect(swarm.x[0]).toBeCloseTo(0.2, 5)
    expect(swarm.z[0]).toBeCloseTo(0.3, 5)
    run(swarm, 1)
    const angles = new Set<number>()
    for (let i = 0; i < swarm.count; i += 50) angles.add(Math.sign(swarm.x[i]) * 2 + Math.sign(swarm.z[i]))
    expect(angles.size).toBeGreaterThan(2)
    expect(meanRadius(swarm)).toBeGreaterThan(0.25)
    expect(meanRadius(swarm)).toBeLessThan(0.9)
  })

  it('no fim do espiral se espalham para longe e somem', () => {
    const swarm = spawned(SENBONZAKURA_PETALS, Math.random)
    run(swarm, 1)
    const orbit = meanRadius(swarm)
    run(swarm, PETAL_LIFE_SEC - 1.15)
    expect(meanRadius(swarm)).toBeGreaterThan(orbit * 1.5)
    run(swarm, 0.3)
    expect(swarm.alive).toBe(false)
    expect(swarm.count).toBe(0)
  })

  it('crescem ao nascer e encolhem até sumir, sem escala negativa', () => {
    const swarm = spawned()
    let min = Number.POSITIVE_INFINITY
    let max = 0
    for (let t = 0; t < PETAL_LIFE_SEC; t += 1 / 60) {
      swarm.step(1 / 60)
      for (let i = 0; i < swarm.count; i += 97) {
        min = Math.min(min, swarm.scale[i])
        max = Math.max(max, swarm.scale[i])
      }
    }
    expect(min).toBeGreaterThanOrEqual(0)
    expect(max).toBeGreaterThan(0.9)
  })

  it('o vento leva as pétalas a favor dele, mais quanto mais tempo no ar', () => {
    const seed = () => {
      let state = 9
      return () => {
        state = (state * 1664525 + 1013904223) % 4294967296
        return state / 4294967296
      }
    }
    const calm = spawned(SENBONZAKURA_PETALS, seed())
    const windy = spawned(SENBONZAKURA_PETALS, seed())
    const meanX = (s: PetalSwarm) => {
      let sum = 0
      for (let i = 0; i < s.count; i++) sum += s.x[i]
      return sum / s.count
    }
    for (let t = 0; t < 2; t += 1 / 60) {
      calm.step(1 / 60)
      windy.step(1 / 60, { x: 0.08, y: 0, z: 0 })
    }
    expect(meanX(windy) - meanX(calm)).toBeGreaterThan(0.1)
  })

  it('não aloca por quadro e ignora dt inválido', () => {
    const swarm = spawned()
    const { x, y, z, scale } = swarm
    for (const dt of [Number.NaN, -1, Number.POSITIVE_INFINITY, 1 / 60]) swarm.step(dt)
    expect(swarm.x).toBe(x)
    expect(swarm.y).toBe(y)
    expect(swarm.z).toBe(z)
    expect(swarm.scale).toBe(scale)
    for (let i = 0; i < swarm.count; i++) expect(Number.isFinite(x[i] + y[i] + z[i] + scale[i])).toBe(true)
  })
})
