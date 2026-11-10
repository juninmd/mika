import { describe, expect, it } from 'vitest'
import {
  FIDGET_COOLDOWN_SEC,
  FIDGET_MAX_GAP_SEC,
  FIDGET_MIN_GAP_SEC,
  FIDGETS,
  type Fidget,
  type FidgetOffsets,
  VrmFidgetLayer,
} from '../vrm-fidget'

const OPEN = { busy: false, enabled: true }
const STEP = 1 / 60

/** Deterministic pseudo-random sequence: a constant rand would pin the pick to one slot. */
function seeded(seed: number): () => number {
  let state = seed
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296
    return state / 4294967296
  }
}

const magnitude = (o: FidgetOffsets) => Object.values(o).reduce((sum, v) => sum + Math.abs(v), 0)

/** Runs the layer for `seconds`, recording each fidget as it starts and the peak offset seen. */
function run(layer: VrmFidgetLayer, seconds: number, gate = OPEN) {
  const started: { fidget: Fidget; at: number }[] = []
  let peak = 0
  let last: Fidget | null = null
  for (let t = 0; t < seconds; t += STEP) {
    const offsets = layer.advance(STEP, gate, 0.6, -0.2)
    peak = Math.max(peak, magnitude(offsets))
    if (layer.current && layer.current !== last) started.push({ fidget: layer.current, at: t })
    last = layer.current
  }
  return { started, peak }
}

// "Ela fica parada entre as poses": micro-idles fill the quiet between full idle actions.
describe('micro-idles do VRM', () => {
  it('nada começa antes do menor intervalo de calma', () => {
    const layer = new VrmFidgetLayer(() => 0)
    const { started } = run(layer, FIDGET_MIN_GAP_SEC - 0.1)
    expect(started).toEqual([])
  })

  it('começa depois do intervalo e mexe o corpo de verdade', () => {
    const { started, peak } = run(new VrmFidgetLayer(() => 0.5), FIDGET_MAX_GAP_SEC + 3)
    expect(started.length).toBeGreaterThan(0)
    expect(peak).toBeGreaterThan(0.02)
  })

  it('nunca repete o mesmo micro-idle duas vezes seguidas', () => {
    const { started } = run(new VrmFidgetLayer(() => 0), 240)
    expect(started.length).toBeGreaterThan(10)
    for (let i = 1; i < started.length; i++) expect(started[i].fidget).not.toBe(started[i - 1].fidget)
  })

  it('respeita o cooldown de cada micro-idle', () => {
    const { started } = run(new VrmFidgetLayer(seeded(7)), 400)
    for (const fidget of FIDGETS) {
      const times = started.filter((s) => s.fidget === fidget).map((s) => s.at)
      for (let i = 1; i < times.length; i++) {
        expect(times[i] - times[i - 1], fidget).toBeGreaterThanOrEqual(FIDGET_COOLDOWN_SEC[fidget] - STEP)
      }
    }
    // Every fidget gets its turn: the cooldowns rotate the pool instead of starving it.
    expect(new Set(started.map((s) => s.fidget)).size).toBe(FIDGETS.length)
  })

  it('entra sem tranco: o primeiro quadro de um micro-idle é quase repouso', () => {
    const layer = new VrmFidgetLayer(() => 0.5)
    let first: number | null = null
    for (let t = 0; t < FIDGET_MAX_GAP_SEC + 1 && first === null; t += STEP) {
      const offsets = layer.advance(STEP, OPEN, 1, 0)
      if (layer.current) first = magnitude(offsets)
    }
    expect(first).not.toBeNull()
    expect(first as number).toBeLessThan(0.005)
  })

  it('cede ao gesto ou à fala: o peso cai suave até zero e nada novo começa', () => {
    const layer = new VrmFidgetLayer(() => 0.5)
    run(layer, FIDGET_MAX_GAP_SEC + 0.8)
    expect(layer.current).not.toBeNull()
    const busy = { busy: true, enabled: true }
    let previous = magnitude(layer.advance(STEP, busy, 0, 0))
    let jump = 0
    for (let i = 0; i < 90; i++) {
      const value = magnitude(layer.advance(STEP, busy, 0, 0))
      jump = Math.max(jump, Math.abs(value - previous))
      previous = value
    }
    expect(jump).toBeLessThan(0.02)
    const { started, peak } = run(layer, 30, busy)
    expect(started).toEqual([])
    expect(peak).toBeLessThan(1e-3)
  })

  it('desligado (reduced motion ou janela em segundo plano) fica parado', () => {
    const { started, peak } = run(new VrmFidgetLayer(() => 0.5), 60, { busy: false, enabled: false })
    expect(started).toEqual([])
    expect(peak).toBe(0)
  })

  it('não aloca por quadro e não envenena os ossos com dt inválido', () => {
    const layer = new VrmFidgetLayer(() => 0.5)
    const offsets = layer.advance(STEP, OPEN, 0, 0)
    for (const dt of [Number.NaN, -1, Number.POSITIVE_INFINITY, STEP]) {
      const again = layer.advance(dt, OPEN, 0, 0)
      expect(again).toBe(offsets)
      for (const value of Object.values(again)) expect(Number.isFinite(value)).toBe(true)
    }
  })
})
