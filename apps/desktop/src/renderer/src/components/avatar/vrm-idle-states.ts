/**
 * Long idle states: when nobody is talking she settles into a held pose for 6–20 s — hands behind
 * her back, arms crossed, a hand on the hip, checking her nails or an invisible phone, humming,
 * stretching, leaning, bouncing on her toes, drowsing late at night. Between the full-pose actions
 * of `vrm-idle.ts` and the micro-fidgets of `vrm-fidget.ts`, this is what keeps a quiet minute from
 * reading as a loop. Pure (no three.js); the offsets buffer is reused every frame.
 */
import { type SpringState, springTo } from './damped-spring'
import { ARM_REST, type ArmPose } from './vrm-body-rig'
import type { IdleGate } from './vrm-idle'

export const IDLE_STATES = [
  'hands-behind-back',
  'arms-crossed',
  'hand-on-hip',
  'check-nails',
  'humming-sway',
  'stretch-up',
  'check-phone',
  'lean-side',
  'toe-bounce',
  'drowsy',
] as const
export type IdleState = (typeof IDLE_STATES)[number]

type Loop = (o: ArmPose, t: number) => void

interface StateSpec {
  pose: Partial<ArmPose>
  /** Seconds held before easing out, drawn uniformly in [min, max]. */
  hold: readonly [number, number]
  /** Relative odds in the pick. */
  odds: number
  /** Motion over the hold, `t` seconds in; added before the blend weight scales everything. */
  loop?: Loop
}

const wave = (t: number, rate: number, phase = 0) => Math.sin(t * rate + phase)

const SPECS: Readonly<Record<IdleState, StateSpec>> = {
  'hands-behind-back': {
    pose: {
      leftUpperArm: -1.3,
      rightUpperArm: 1.32,
      leftUpperArmX: 0.42,
      rightUpperArmX: 0.42,
      leftLowerArm: -0.55,
      rightLowerArm: 0.55,
      leftLowerArmY: 1.25,
      rightLowerArmY: -1.25,
      leftGrip: 0.45,
      rightGrip: 0.45,
      chestX: -0.05,
      headZ: 0.1,
    },
    hold: [8, 16],
    odds: 1.2,
    loop: (o, t) => {
      o.spineZ += wave(t, 0.9) * 0.03
      o.headZ += wave(t, 0.9, -0.6) * 0.04
    },
  },
  'arms-crossed': {
    pose: {
      leftUpperArm: -1.12,
      rightUpperArm: 1.12,
      leftUpperArmX: -0.72,
      rightUpperArmX: -0.72,
      leftLowerArm: -2.05,
      rightLowerArm: 1.95,
      leftLowerArmY: 1.3,
      rightLowerArmY: -1.2,
      leftGrip: 0.5,
      rightGrip: 0.5,
      chestX: -0.03,
      headZ: -0.08,
      spineZ: -0.05,
    },
    hold: [8, 18],
    odds: 1.1,
    loop: (o, t) => {
      o.headY += wave(t, 0.35) * 0.1
    },
  },
  'hand-on-hip': {
    pose: {
      rightUpperArm: 0.72,
      rightUpperArmX: 0.3,
      rightLowerArm: 1.55,
      rightLowerArmY: -0.4,
      rightHandZ: 0.45,
      rightGrip: 0.7,
      spineZ: -0.2,
      headZ: 0.12,
      chestX: -0.03,
    },
    hold: [7, 14],
    odds: 1,
    loop: (o, t) => {
      o.headY += wave(t, 0.5) * 0.06
    },
  },
  'check-nails': {
    pose: {
      rightUpperArm: 0.95,
      rightUpperArmX: -1.3,
      rightLowerArm: 1.75,
      rightLowerArmY: -0.9,
      rightHandX: -0.7,
      rightGrip: 0.85,
      headX: 0.14,
      headY: 0.12,
      headZ: -0.05,
    },
    hold: [6, 11],
    odds: 0.8,
    loop: (o, t) => {
      // Turns the hand to catch the light, then back.
      o.rightHandZ += wave(t, 0.8) * 0.3
      o.rightLowerArmY += wave(t, 0.8, 0.4) * 0.15
    },
  },
  'humming-sway': {
    pose: { leftGrip: 0.35, rightGrip: 0.35, headX: -0.04 },
    hold: [8, 16],
    odds: 1,
    loop: (o, t) => {
      o.spineZ += wave(t, 1.9) * 0.06
      o.headZ += wave(t, 1.9, -0.7) * 0.1
      o.leftUpperArm -= wave(t, 1.9) * 0.05
      o.rightUpperArm -= wave(t, 1.9) * 0.05
    },
  },
  'stretch-up': {
    pose: {
      leftUpperArm: 1.2,
      rightUpperArm: -1.2,
      leftLowerArm: 0.55,
      rightLowerArm: -0.55,
      leftUpperArmX: -0.25,
      rightUpperArmX: -0.25,
      leftHandX: 0.6,
      rightHandX: 0.6,
      leftGrip: 0.2,
      rightGrip: 0.2,
      chestX: -0.1,
      headX: -0.14,
    },
    hold: [6, 9],
    odds: 0.7,
    loop: (o, t) => {
      o.spineZ += wave(t, 0.7) * 0.1
      o.headZ -= wave(t, 0.7) * 0.05
    },
  },
  'check-phone': {
    pose: {
      leftUpperArm: -1.18,
      leftUpperArmX: -0.62,
      leftLowerArm: -1.55,
      leftLowerArmY: 0.75,
      leftHandX: -0.5,
      leftGrip: 0.55,
      rightUpperArm: 1.18,
      rightUpperArmX: -0.55,
      rightLowerArm: 1.35,
      rightLowerArmY: -0.8,
      rightHandX: -0.4,
      rightGrip: 0.6,
      headX: 0.3,
      chestX: 0.06,
    },
    hold: [8, 18],
    odds: 1,
    loop: (o, t) => {
      // A scroll now and then, and the odd glance up from the screen.
      o.rightHandZ += Math.max(0, wave(t, 2.6)) ** 3 * 0.12
      o.headX -= Math.max(0, wave(t, 0.45, -1.4)) ** 6 * 0.25
    },
  },
  'lean-side': {
    pose: {
      spineZ: -0.22,
      chestX: -0.02,
      headZ: 0.16,
      leftUpperArm: -1.38,
      rightUpperArm: 1.3,
      rightUpperArmX: 0.1,
      rightLowerArmY: 0.1,
    },
    hold: [7, 14],
    odds: 0.9,
    loop: (o, t) => {
      o.headY += wave(t, 0.4) * 0.07
    },
  },
  'toe-bounce': {
    pose: {
      leftUpperArm: -1.3,
      rightUpperArm: 1.32,
      leftUpperArmX: 0.4,
      rightUpperArmX: 0.4,
      leftLowerArm: -0.55,
      rightLowerArm: 0.55,
      leftLowerArmY: 1.25,
      rightLowerArmY: -1.25,
      headZ: 0.06,
    },
    hold: [6, 10],
    odds: 0.7,
    loop: (o, t) => {
      const beat = 0.5 - 0.5 * Math.cos(t * 5.5)
      o.hipsY += beat * 0.018
      o.leftLowerLegX += beat * 0.08
      o.rightLowerLegX += beat * 0.08
      o.headZ += wave(t, 2.75) * 0.05
    },
  },
  drowsy: {
    pose: {
      leftUpperArm: -1.42,
      rightUpperArm: 1.44,
      leftLowerArm: -0.05,
      rightLowerArm: 0.05,
      leftGrip: 0.15,
      rightGrip: 0.15,
      headX: 0.2,
      chestX: 0.07,
      headZ: 0.1,
    },
    hold: [10, 20],
    // Only late at night: `BAND_ODDS` zeroes it the rest of the day.
    odds: 1,
    loop: (o, t) => {
      // Nods off, catches herself.
      o.headX += Math.max(0, wave(t, 0.6)) ** 4 * 0.2
    },
  },
}

export const IDLE_STATE_HOLD_SEC: Readonly<Record<IdleState, readonly [number, number]>> = Object.fromEntries(
  IDLE_STATES.map((s) => [s, SPECS[s].hold]),
) as Record<IdleState, readonly [number, number]>
/** Start-to-start, per state: the same pose again within a minute reads as a loop. */
export const IDLE_STATE_COOLDOWN_SEC = 60
export const IDLE_STATE_MIN_GAP_SEC = 8
export const IDLE_STATE_MAX_GAP_SEC = 20
/** The skin stage is a showroom: a visitor sees a state within seconds, not a minute. */
const SHOWCASE_GAP_SEC: readonly [number, number] = [2, 5]
/** ~2 s in and out; an interruption takes ~1.3 s so a gesture or a reply is never delayed. */
const ENTER_OMEGA = 2.4
const INTERRUPT_OMEGA = 5.5
const GONE = 0.01

type Band = 'night' | 'morning' | 'evening' | 'day'

/** Time-of-day flavour, as odds multipliers: sleepy late at night, stretching in the morning, humming in the evening. */
const BAND_ODDS: Readonly<Record<Band, Partial<Record<IdleState, number>>>> = {
  night: { drowsy: 3, 'stretch-up': 1.6, 'toe-bounce': 0.3, 'humming-sway': 0.3 },
  morning: { drowsy: 0, 'stretch-up': 2.5 },
  evening: { drowsy: 0, 'humming-sway': 2 },
  day: { drowsy: 0 },
}

function band(hour: number): Band {
  if (hour >= 23 || hour < 5) return 'night'
  if (hour >= 6 && hour < 11) return 'morning'
  return hour >= 18 ? 'evening' : 'day'
}

function hourOdds(state: IdleState, hour: number): number {
  return SPECS[state].odds * (BAND_ODDS[band(hour)][state] ?? 1)
}

interface Channel {
  key: keyof ArmPose
  offset: number
}

const POSE_KEYS = Object.keys(ARM_REST) as (keyof ArmPose)[]
// Resolved once: the frame only walks these lists.
const CHANNELS = {} as Record<IdleState, readonly Channel[]>
for (const state of IDLE_STATES) {
  CHANNELS[state] = (Object.entries(SPECS[state].pose) as [keyof ArmPose, number][]).map(([key, target]) => ({
    key,
    offset: target - ARM_REST[key],
  }))
}

const unit = (value: number) => (Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0)

export class VrmIdleStateLayer {
  /** Offsets from `ARM_REST`, already scaled by the blend weight. Reused; read, never keep. */
  readonly offsets: ArmPose = Object.fromEntries(POSE_KEYS.map((k) => [k, 0])) as unknown as ArmPose
  private shown: IdleState | null = null
  private holding = false
  private interrupted = false
  private elapsed = 0
  private holdFor = 0
  private quiet = 0
  private clock = 0
  private gap: number
  private showcase = false
  private recent: [IdleState | null, IdleState | null] = [null, null]
  private readonly weight: SpringState = { value: 0, velocity: 0 }
  private readonly startedAt = Object.fromEntries(IDLE_STATES.map((s) => [s, Number.NEGATIVE_INFINITY])) as Record<
    IdleState,
    number
  >

  constructor(private readonly rand: () => number = Math.random) {
    this.gap = this.randomGap()
  }

  /** The state on screen, entering, held or easing out; null once it has fully left. */
  get current(): IdleState | null {
    return this.shown
  }

  /** Enters `state` now (debug bridge / E2E), skipping the quiet gap; it still yields to speech. */
  play(state: IdleState): void {
    this.clear()
    this.start(state)
  }

  setShowcase(showcase: boolean): void {
    this.showcase = showcase
    this.gap = Math.min(this.gap, this.randomGap())
  }

  advance(dt: number, gate: IdleGate, hour: number): ArmPose {
    const step = Number.isFinite(dt) ? Math.min(Math.max(0, dt), 0.1) : 0
    this.clock += step
    const open = gate.enabled && !gate.busy
    if (this.holding && !open) this.release(true)
    this.schedule(step, open, gate.enabled, hour)
    const weight = springTo(this.weight, this.holding ? 1 : 0, this.interrupted ? INTERRUPT_OMEGA : ENTER_OMEGA, step)
    if (!this.holding && this.shown && weight < GONE) this.clear()
    this.write(weight)
    return this.offsets
  }

  private schedule(step: number, open: boolean, enabled: boolean, hour: number): void {
    if (this.shown) {
      this.elapsed += step
      if (this.holding && this.elapsed >= this.holdFor) this.release(false)
      return
    }
    if (!enabled) this.quiet = 0
    if (!open) return
    this.quiet += step
    if (this.quiet < this.gap) return
    this.quiet = 0
    this.gap = this.randomGap()
    this.start(this.pick(hour))
  }

  private start(state: IdleState | null): void {
    if (!state) return
    const [min, max] = SPECS[state].hold
    this.shown = state
    this.holding = true
    this.interrupted = false
    this.elapsed = 0
    this.holdFor = min + unit(this.rand()) * (max - min)
    this.startedAt[state] = this.clock
    this.recent = [state, this.recent[0]]
  }

  private release(interrupted: boolean): void {
    this.holding = false
    this.interrupted = interrupted
  }

  private clear(): void {
    this.shown = null
    this.weight.value = 0
    this.weight.velocity = 0
  }

  private eligible(state: IdleState, hour: number): number {
    if (state === this.recent[0] || state === this.recent[1]) return 0
    if (this.clock - this.startedAt[state] < IDLE_STATE_COOLDOWN_SEC) return 0
    return hourOdds(state, hour)
  }

  /** Weighted over the states off cooldown and not among the last two; walked twice, no filtered copy. */
  private pick(hour: number): IdleState | null {
    let total = 0
    for (const state of IDLE_STATES) total += this.eligible(state, hour)
    if (total <= 0) return null
    let roll = unit(this.rand()) * total
    for (const state of IDLE_STATES) {
      const odds = this.eligible(state, hour)
      if (odds <= 0) continue
      if (roll < odds) return state
      roll -= odds
    }
    return null
  }

  private write(weight: number): void {
    const o = this.offsets
    for (const key of POSE_KEYS) o[key] = 0
    const state = this.shown
    if (!state) return
    for (const channel of CHANNELS[state]) o[channel.key] = channel.offset
    SPECS[state].loop?.(o, this.elapsed)
    for (const key of POSE_KEYS) o[key] *= weight
  }

  private randomGap(): number {
    const [min, max] = this.showcase ? SHOWCASE_GAP_SEC : [IDLE_STATE_MIN_GAP_SEC, IDLE_STATE_MAX_GAP_SEC]
    return min + unit(this.rand()) * (max - min)
  }
}
