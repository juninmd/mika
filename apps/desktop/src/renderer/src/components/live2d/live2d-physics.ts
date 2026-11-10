export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t
}

export function clamp(val: number, min: number, max: number): number {
  return Math.min(Math.max(val, min), max)
}

export const MAX_FRAME_DELTA_SECONDS = 1 / 30

/** Teto do alcance depois de uma pausa: simular segundos perdidos só faria o avatar "correr". */
const MAX_CATCHUP_SECONDS = 0.25

/** Divide o quadro em passos estáveis (≤ MAX_FRAME_DELTA_SECONDS) sem perder tempo real. */
export function frameSubsteps(rawDt: number): { steps: number; step: number } {
  if (!Number.isFinite(rawDt) || rawDt <= 0) return { steps: 0, step: 0 }
  const dt = Math.min(rawDt, MAX_CATCHUP_SECONDS)
  const steps = Math.ceil(dt / MAX_FRAME_DELTA_SECONDS - 1e-9)
  return { steps, step: dt / steps }
}

export function map(val: number, inMin: number, inMax: number, outMin: number, outMax: number): number {
  return ((val - inMin) * (outMax - outMin)) / (inMax - inMin) + outMin
}

export class Spring {
  current: number
  target: number
  velocity: number
  stiffness: number
  damping: number
  mass: number

  constructor(initial = 0, stiffness = 120, damping = 10, mass = 1) {
    this.current = initial
    this.target = initial
    this.velocity = 0
    this.stiffness = stiffness
    this.damping = damping
    this.mass = mass
  }

  update(dt: number) {
    if (dt <= 0) return

    const displacement = this.current - this.target
    const springForce = -this.stiffness * displacement

    const dampingForce = -this.damping * this.velocity

    const force = springForce + dampingForce

    const acceleration = force / this.mass

    this.velocity += acceleration * dt
    this.current += this.velocity * dt
  }

  reset(val: number) {
    this.current = val
    this.target = val
    this.velocity = 0
  }
}

export class PerlinNoise1D {
  private permutation: number[] = []

  constructor() {
    this.seed(Math.random())
  }

  seed(val: number) {
    const p: number[] = []
    for (let i = 0; i < 256; i++) {
      p[i] = i
    }

    let seed = Math.floor(val * 65536)
    for (let i = 255; i > 0; i--) {
      seed = (seed * 1664525 + 1013904223) % 4294967296
      const j = Math.floor((seed / 4294967296) * (i + 1))
      const temp = p[i]
      p[i] = p[j]
      p[j] = temp
    }
    this.permutation = [...p, ...p]
  }

  get(x: number): number {
    const X = Math.floor(x) & 255
    const xf = x - Math.floor(x)

    const u = xf * xf * xf * (xf * (xf * 6 - 15) + 10)

    const g0 = (this.permutation[X] & 1) === 0 ? 1 : -1
    const g1 = (this.permutation[X + 1] & 1) === 0 ? 1 : -1

    const n0 = g0 * xf
    const n1 = g1 * (xf - 1)

    return lerp(n0, n1, u)
  }
}

export class EnvelopeFollower {
  value = 0

  attackMs: number

  releaseMs: number

  constructor(attackMs = 25, releaseMs = 80) {
    this.attackMs = attackMs
    this.releaseMs = releaseMs
  }

  update(target: number, dt: number) {
    const diff = target - this.value
    const rate = 1000 / (diff > 0 ? this.attackMs : this.releaseMs)
    this.value += diff * (1 - Math.exp(-dt * rate))
    return this.value
  }
}
