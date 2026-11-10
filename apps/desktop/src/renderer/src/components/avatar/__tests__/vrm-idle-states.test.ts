import { describe, expect, it } from 'vitest'
import type { ArmPose } from '../vrm-body-rig'
import {
  IDLE_STATE_COOLDOWN_SEC,
  IDLE_STATE_HOLD_SEC,
  IDLE_STATE_MIN_GAP_SEC,
  IDLE_STATES,
  type IdleState,
  VrmIdleStateLayer,
} from '../vrm-idle-states'

const OPEN = { busy: false, enabled: true }
const STEP = 1 / 30

function seeded(seed: number): () => number {
  let state = seed
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296
    return state / 4294967296
  }
}

const magnitude = (o: ArmPose) => Object.values(o).reduce((sum, v) => sum + Math.abs(v), 0)

interface Run {
  started: { state: IdleState; at: number }[]
  ended: { state: IdleState; at: number }[]
  peak: number
  /** Largest change of any one channel between frames (rad, or metres for the hips). */
  maxJump: number
}

/** Runs the layer, recording each state's start and end, the peak offset and the biggest per-frame jump. */
function run(layer: VrmIdleStateLayer, seconds: number, gate = OPEN, hour = 14): Run {
  const out: Run = { started: [], ended: [], peak: 0, maxJump: 0 }
  let last: IdleState | null = null
  const previous = { ...layer.offsets }
  for (let t = 0; t < seconds; t += STEP) {
    const offsets = layer.advance(STEP, gate, hour)
    out.peak = Math.max(out.peak, magnitude(offsets))
    for (const key of Object.keys(previous) as (keyof ArmPose)[]) {
      out.maxJump = Math.max(out.maxJump, Math.abs(offsets[key] - previous[key]))
      previous[key] = offsets[key]
    }
    if (layer.current !== last) {
      if (last) out.ended.push({ state: last, at: t })
      if (layer.current) out.started.push({ state: layer.current, at: t })
    }
    last = layer.current
  }
  return out
}

// Pedido do usuário (2026-09-28): quando ninguém fala ela precisa parecer viva — estados longos
// (braços cruzados, mão na cintura, olhar as unhas...) além dos micro-fidgets.
describe('estados ociosos longos do VRM', () => {
  it('nada começa antes do intervalo mínimo de calma', () => {
    const { started } = run(new VrmIdleStateLayer(() => 0), IDLE_STATE_MIN_GAP_SEC - 0.2)
    expect(started).toEqual([])
  })

  it('cada estado segura entre 6 e 20 s e entra e sai sem tranco', () => {
    const r = run(new VrmIdleStateLayer(seeded(3)), 600)
    expect(r.started.length).toBeGreaterThan(8)
    for (const [state, [min, max]] of Object.entries(IDLE_STATE_HOLD_SEC)) {
      expect(min, state).toBeGreaterThanOrEqual(6)
      expect(max, state).toBeLessThanOrEqual(20)
    }
    // Held time plus the damped exit: a state is on screen at least its minimum hold.
    for (let i = 0; i < r.ended.length; i++) {
      const lasted = r.ended[i].at - r.started[i].at
      expect(lasted, r.started[i].state).toBeGreaterThanOrEqual(IDLE_STATE_HOLD_SEC[r.started[i].state][0])
      expect(lasted, r.started[i].state).toBeLessThan(IDLE_STATE_HOLD_SEC[r.started[i].state][1] + 6)
    }
    expect(r.peak).toBeGreaterThan(0.5)
    expect(r.maxJump).toBeLessThan(0.1)
  })

  it('não repete o estado recente e respeita o cooldown de cada um', () => {
    const { started } = run(new VrmIdleStateLayer(seeded(11)), 1800)
    for (let i = 2; i < started.length; i++) {
      expect(started[i].state).not.toBe(started[i - 1].state)
      expect(started[i].state).not.toBe(started[i - 2].state)
    }
    for (const state of IDLE_STATES) {
      const times = started.filter((s) => s.state === state).map((s) => s.at)
      for (let i = 1; i < times.length; i++) {
        expect(times[i] - times[i - 1], state).toBeGreaterThanOrEqual(IDLE_STATE_COOLDOWN_SEC - STEP)
      }
    }
    // Daytime pool: everything but the late-night drowse gets its turn.
    expect(new Set(started.map((s) => s.state)).size).toBe(IDLE_STATES.length - 1)
    expect(started.some((s) => s.state === 'drowsy')).toBe(false)
  })

  it('de madrugada o sono entra no sorteio', () => {
    const { started } = run(new VrmIdleStateLayer(seeded(5)), 1800, OPEN, 2)
    expect(started.some((s) => s.state === 'drowsy')).toBe(true)
  })

  it('fala ou gesto interrompe: o peso cai suave a zero em pouco mais de um segundo', () => {
    const layer = new VrmIdleStateLayer(() => 0.5)
    run(layer, IDLE_STATE_MIN_GAP_SEC + 25)
    for (let i = 0; i < 300 && !layer.current; i++) layer.advance(STEP, OPEN, 14)
    run(layer, 2.5)
    expect(layer.current).not.toBeNull()
    const busy = run(layer, 1.5, { busy: true, enabled: true })
    expect(busy.maxJump).toBeLessThan(0.15)
    expect(layer.current).toBeNull()
    expect(magnitude(layer.offsets)).toBeLessThan(1e-6)
    expect(run(layer, 60, { busy: true, enabled: true }).started).toEqual([])
  })

  it('desligado (movimento reduzido ou janela em segundo plano) fica parado', () => {
    const r = run(new VrmIdleStateLayer(() => 0.5), 120, { busy: false, enabled: false })
    expect(r.started).toEqual([])
    expect(r.peak).toBe(0)
  })

  it('na vitrine da skin o primeiro estado chega logo', () => {
    const layer = new VrmIdleStateLayer(() => 0)
    layer.setShowcase(true)
    expect(run(layer, 4).started.length).toBe(1)
  })

  it('não aloca por quadro e não envenena os ossos com dt inválido', () => {
    const layer = new VrmIdleStateLayer(() => 0.5)
    const offsets = layer.advance(STEP, OPEN, 14)
    run(layer, IDLE_STATE_MIN_GAP_SEC + 25)
    for (const dt of [Number.NaN, -1, Number.POSITIVE_INFINITY, STEP]) {
      const again = layer.advance(dt, OPEN, 14)
      expect(again).toBe(offsets)
      for (const value of Object.values(again)) expect(Number.isFinite(value)).toBe(true)
    }
  })
})
