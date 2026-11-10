/** Ambient idle motion for VRM, mirroring Live2D's `updateIdleMotions`: without it the avatar only ever moves on explicit triggers. */
import type { VrmBodyAction } from './vrm-body-rig'

export const IDLE_MOTION_MIN_INTERVAL_SEC = 3
export const IDLE_MOTION_MAX_INTERVAL_SEC = 8

// Subtle, ambient poses only; reactive ones (wave, cheer, heart, victory, salute, bow...) would read as
// a response to something that didn't happen if fired at random.
const IDLE_ACTIONS: readonly VrmBodyAction[] = [
  'stretch',
  'sigh',
  'curious',
  'relaxed',
  'thinking',
  'shrug',
  'smug',
  'yawn',
  'drink',
]

function unitInterval(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}

function pick(pool: readonly VrmBodyAction[], rand: number, previous?: VrmBodyAction): VrmBodyAction {
  const choices = previous ? pool.filter((action) => action !== previous) : pool
  const index = Math.min(choices.length - 1, Math.floor(unitInterval(rand) * choices.length))
  return choices[index]
}

export function randomIdleAction(rand: number, previous?: VrmBodyAction): VrmBodyAction {
  return pick(IDLE_ACTIONS, rand, previous)
}

function randomInterval(rand: () => number): number {
  return (
    IDLE_MOTION_MIN_INTERVAL_SEC + unitInterval(rand()) * (IDLE_MOTION_MAX_INTERVAL_SEC - IDLE_MOTION_MIN_INTERVAL_SEC)
  )
}

const SLEEPY_ACTIONS: readonly VrmBodyAction[] = ['yawn', 'stretch', 'doze']
/** Share of late-night idle picks that are a yawn, a stretch or nodding off. */
const SLEEPY_SHARE = 0.5

/** 23h to 5h: past bedtime she is sleepy too. */
export function isSleepyHour(hour: number): boolean {
  return hour >= 23 || hour < 5
}

export interface IdleGate {
  /** An explicit action or speech already owns the body; a fresh quiet interval starts afterwards. */
  busy: boolean
  /** False while backgrounded/reduced-motion, per `resolveEffectQuality()`. */
  enabled: boolean
}

export class VrmIdleScheduler {
  private timer = 0
  private nextInterval: number
  private previous?: VrmBodyAction

  constructor(
    private readonly rand: () => number = Math.random,
    private readonly hour: () => number = () => new Date().getHours(),
  ) {
    this.nextInterval = randomInterval(this.rand)
  }

  /** Advances the clock; returns the pose to play once it expires, or null otherwise. */
  tick(dt: number, gate: IdleGate): VrmBodyAction | null {
    if (gate.busy || !gate.enabled) {
      this.timer = 0
      return null
    }
    this.timer += Number.isFinite(dt) ? Math.max(0, dt) : 0
    if (this.timer < this.nextInterval) return null
    this.timer = 0
    this.nextInterval = randomInterval(this.rand)
    this.previous =
      isSleepyHour(this.hour()) && this.rand() < SLEEPY_SHARE
        ? pick(SLEEPY_ACTIONS, this.rand(), this.previous)
        : randomIdleAction(this.rand(), this.previous)
    return this.previous
  }
}
