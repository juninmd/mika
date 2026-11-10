/**
 * Micro-idles: small, additive fidgets (weight shift, a look around, fiddling with the hands, a
 * glance at the cursor, a neck tilt) between the full idle poses of `vrm-idle.ts`. Without them the
 * body only ever breathes on a loop between poses. Pure — no three.js — and no per-frame allocation.
 */
import { type SpringState, springTo } from './damped-spring'
import type { IdleGate } from './vrm-idle'
import { bell, type Keyframes, sampleKeys } from './vrm-keyframes'

export const FIDGETS = ['weight-shift', 'look-around', 'hand-fidget', 'glance-cursor', 'neck-tilt'] as const
export type Fidget = (typeof FIDGETS)[number]

const FIDGET_SECONDS: Readonly<Record<Fidget, number>> = {
  'weight-shift': 3.4,
  'look-around': 3.8,
  'hand-fidget': 2.6,
  'glance-cursor': 2.2,
  'neck-tilt': 2.8,
}

/** Minimum time between two starts of the same fidget: the eye catches a tic repeated too soon. */
export const FIDGET_COOLDOWN_SEC: Readonly<Record<Fidget, number>> = {
  'weight-shift': 10,
  'look-around': 16,
  'hand-fidget': 12,
  'glance-cursor': 9,
  'neck-tilt': 18,
}

export const FIDGET_MIN_GAP_SEC = 2.5
export const FIDGET_MAX_GAP_SEC = 6
/** Blend stiffness: ~1 s to fully yield to a gesture or speech, critically damped (no pop). */
const BLEND_OMEGA = 5
const YIELDED = 0.02

/** Radians on the normalized bones, plus a sideways body shift in metres on the model root. */
export interface FidgetOffsets {
  headX: number
  headY: number
  headZ: number
  spineZ: number
  chestX: number
  leftHandX: number
  rightHandX: number
  leftHandZ: number
  rightHandZ: number
  bodyShift: number
}

const LOOK_AROUND: Keyframes = [
  [0, 0],
  [0.22, 0.3],
  [0.45, 0.3],
  [0.68, -0.22],
  [0.85, -0.22],
  [1, 0],
]

const GLANCE: Keyframes = [
  [0, 0],
  [0.15, 1],
  [0.7, 1],
  [1, 0],
]

const unit = (value: number) => (Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0)

function clear(o: FidgetOffsets): void {
  o.headX = o.headY = o.headZ = o.spineZ = o.chestX = 0
  o.leftHandX = o.rightHandX = o.leftHandZ = o.rightHandZ = o.bodyShift = 0
}

/** Writes the raw (unblended) curve of `fidget` at `p` into `o`, which starts cleared. */
function shape(o: FidgetOffsets, fidget: Fidget, p: number, side: number, cursorX: number, cursorY: number): void {
  const b = bell(p)
  if (fidget === 'weight-shift') {
    o.bodyShift = side * 0.02 * b
    o.spineZ = -side * 0.04 * b
    o.headZ = side * 0.035 * b
  } else if (fidget === 'look-around') {
    o.headY = side * sampleKeys(LOOK_AROUND, p)
    o.headX = -0.04 * b
  } else if (fidget === 'hand-fidget') {
    o.rightHandZ = Math.sin(p * Math.PI * 6) * 0.22 * b
    o.leftHandX = 0.18 * b
    o.rightHandX = -0.12 * b
    o.headX = 0.08 * b
  } else if (fidget === 'glance-cursor') {
    const glance = sampleKeys(GLANCE, p)
    const x = Math.abs(cursorX) < 0.15 ? side : Math.max(-1, Math.min(1, cursorX))
    o.headY = x * 0.3 * glance
    o.headX = Math.max(-1, Math.min(1, cursorY)) * 0.12 * glance
  } else {
    o.headZ = side * 0.12 * b
    o.spineZ = -side * 0.015 * b
  }
}

export class VrmFidgetLayer {
  /** Reused every frame; read it, never keep it. */
  readonly offsets: FidgetOffsets = {
    headX: 0,
    headY: 0,
    headZ: 0,
    spineZ: 0,
    chestX: 0,
    leftHandX: 0,
    rightHandX: 0,
    leftHandZ: 0,
    rightHandZ: 0,
    bodyShift: 0,
  }
  private active: Fidget | null = null
  private last: Fidget | null = null
  private elapsed = 0
  private side = 1
  private clock = 0
  private quiet = 0
  private nextGap: number
  private readonly weight: SpringState = { value: 0, velocity: 0 }
  private readonly startedAt: Record<Fidget, number> = {
    'weight-shift': Number.NEGATIVE_INFINITY,
    'look-around': Number.NEGATIVE_INFINITY,
    'hand-fidget': Number.NEGATIVE_INFINITY,
    'glance-cursor': Number.NEGATIVE_INFINITY,
    'neck-tilt': Number.NEGATIVE_INFINITY,
  }

  constructor(private readonly rand: () => number = Math.random) {
    this.nextGap = this.randomGap()
  }

  get current(): Fidget | null {
    return this.active
  }

  /** Advances the clock and returns this frame's offsets, already scaled by the blend weight. */
  advance(dt: number, gate: IdleGate, cursorX: number, cursorY: number): FidgetOffsets {
    const step = Number.isFinite(dt) ? Math.min(Math.max(0, dt), 0.1) : 0
    const open = gate.enabled && !gate.busy
    this.clock += step
    const weight = springTo(this.weight, open ? 1 : 0, BLEND_OMEGA, step)
    if (!open && weight < YIELDED) this.yieldFully()
    this.schedule(step, open)
    clear(this.offsets)
    if (this.active) {
      shape(this.offsets, this.active, this.elapsed / FIDGET_SECONDS[this.active], this.side, cursorX, cursorY)
      scale(this.offsets, weight)
    }
    return this.offsets
  }

  private schedule(step: number, open: boolean): void {
    if (this.active) {
      this.elapsed += step
      if (this.elapsed < FIDGET_SECONDS[this.active]) return
      this.active = null
    }
    if (!open) {
      this.quiet = 0
      return
    }
    this.quiet += step
    if (this.quiet < this.nextGap) return
    this.quiet = 0
    this.nextGap = this.randomGap()
    this.start(this.pick())
  }

  private start(fidget: Fidget | null): void {
    if (!fidget) return
    this.active = fidget
    this.last = fidget
    this.elapsed = 0
    this.side = this.rand() < 0.5 ? -1 : 1
    this.startedAt[fidget] = this.clock
  }

  private eligible(fidget: Fidget): boolean {
    return fidget !== this.last && this.clock - this.startedAt[fidget] >= FIDGET_COOLDOWN_SEC[fidget]
  }

  /** Uniform over the fidgets off cooldown and not just played; counted twice to avoid a filtered copy. */
  private pick(): Fidget | null {
    let count = 0
    for (const fidget of FIDGETS) if (this.eligible(fidget)) count++
    if (count === 0) return null
    let index = Math.min(count - 1, Math.floor(unit(this.rand()) * count))
    for (const fidget of FIDGETS) {
      if (!this.eligible(fidget)) continue
      if (index === 0) return fidget
      index--
    }
    return null
  }

  private yieldFully(): void {
    this.active = null
    this.weight.value = 0
    this.weight.velocity = 0
  }

  private randomGap(): number {
    return FIDGET_MIN_GAP_SEC + unit(this.rand()) * (FIDGET_MAX_GAP_SEC - FIDGET_MIN_GAP_SEC)
  }
}

function scale(o: FidgetOffsets, w: number): void {
  o.headX *= w
  o.headY *= w
  o.headZ *= w
  o.spineZ *= w
  o.chestX *= w
  o.leftHandX *= w
  o.rightHandX *= w
  o.leftHandZ *= w
  o.rightHandZ *= w
  o.bodyShift *= w
}
