/**
 * Pure choreography of the senbonzakura move — the blade's timeline and the petal swarm it dissolves
 * into. No three.js here: `vrm-katana.ts` owns the meshes and only reads these numbers each frame.
 * The swarm lives in preallocated typed arrays; spawning and stepping allocate nothing.
 */

/** Phase marks, as action progress (0..1): draw, raised before the face, hold, turn down, "Chire". */
export const SENBONZAKURA_TIMELINE = {
  drawAt: 0.08,
  raisedAt: 0.3,
  flipFrom: 0.38,
  flipTo: 0.46,
  scatterAt: 0.48,
  /** The hilt dissolves with the petals (~0.4 s), not long after them. */
  hiltGoneAt: 0.56,
} as const

export const PETAL_CAPACITY = 1200
export const SENBONZAKURA_PETALS = 1000
/** Reduced motion: the pose still plays, the storm is a handful of petals. */
export const REDUCED_PETALS = 120
export const PETAL_LIFE_SEC = 2.6
const GROW_SEC = 0.25
const GATHER_SEC = 0.7
const SCATTER_SEC = 1.2
/** Metres of drift per second per unit of wind (`vrm-wind.ts` units, WIND_MAX = 0.1). */
const PETAL_WIND_DRIFT = 6

const T = SENBONZAKURA_TIMELINE

const clamp01 = (t: number) => (t > 0 ? Math.min(t, 1) : 0)
const smooth = (t: number) => {
  const x = clamp01(t)
  return x * x * (3 - 2 * x)
}

export interface BladePose {
  /** Katana scale: grows in on the draw, dissolves right after the release. */
  hilt: number
  /** 0 = drawn low at the side, 1 = upright before the face. */
  raise: number
  /** Second hand on the hilt: joins as the blade rises, lets go as it turns down. */
  join: number
  /** 0 = upright in both hands, 1 = let go and turned point-down for the release. */
  flip: number
  bladeVisible: boolean
}

export function bladePoseInto(out: BladePose, progress: number): BladePose {
  const p = Number.isFinite(progress) ? progress : 0
  const inside = p > 0 && p < 1
  out.hilt = inside ? smooth((p - T.drawAt) / 0.08) * (1 - smooth((p - T.scatterAt) / (T.hiltGoneAt - T.scatterAt))) : 0
  out.raise = smooth((p - (T.drawAt + 0.02)) / (T.raisedAt - T.drawAt - 0.02))
  out.flip = smooth((p - T.flipFrom) / (T.flipTo - T.flipFrom))
  out.join = inside ? smooth((p - T.drawAt - 0.06) / (T.raisedAt - T.drawAt - 0.08)) * (1 - out.flip) : 0
  out.bladeVisible = inside && p < T.scatterAt
  return out
}

export interface Point3 {
  x: number
  y: number
  z: number
}

/** Petals orbit a vertical axis through `center`, then spiral out and fade. */
export class PetalSwarm {
  readonly x = new Float32Array(PETAL_CAPACITY)
  readonly y = new Float32Array(PETAL_CAPACITY)
  readonly z = new Float32Array(PETAL_CAPACITY)
  /** Euler tumble per petal (radians). */
  readonly rx = new Float32Array(PETAL_CAPACITY)
  readonly ry = new Float32Array(PETAL_CAPACITY)
  readonly rz = new Float32Array(PETAL_CAPACITY)
  readonly scale = new Float32Array(PETAL_CAPACITY)
  count = 0
  private age = -1
  private centerX = 0
  private centerZ = 0
  private readonly startX = new Float32Array(PETAL_CAPACITY)
  private readonly startY = new Float32Array(PETAL_CAPACITY)
  private readonly startZ = new Float32Array(PETAL_CAPACITY)
  private readonly angle = new Float32Array(PETAL_CAPACITY)
  private readonly spin = new Float32Array(PETAL_CAPACITY)
  private readonly radius = new Float32Array(PETAL_CAPACITY)
  private readonly rise = new Float32Array(PETAL_CAPACITY)
  private readonly tumble = new Float32Array(PETAL_CAPACITY)
  private readonly point: Point3 = { x: 0, y: 0, z: 0 }
  /** Wind drift the whole swarm has built up since it was born (m). */
  private driftX = 0
  private driftY = 0
  private driftZ = 0

  get alive(): boolean {
    return this.age >= 0
  }

  /** `origin(i, out)` writes where petal `i` is born (a point along the dissolving blade). */
  spawn(
    count: number,
    origin: (i: number, out: Point3) => void,
    center: { x: number; z: number },
    rand: () => number = Math.random,
  ): void {
    this.count = Math.max(0, Math.min(PETAL_CAPACITY, Math.floor(count)))
    this.centerX = center.x
    this.centerZ = center.z
    for (let i = 0; i < this.count; i++) {
      origin(i, this.point)
      this.startX[i] = this.point.x
      this.startY[i] = this.point.y
      this.startZ[i] = this.point.z
      this.angle[i] = rand() * Math.PI * 2
      this.spin[i] = (1.6 + rand() * 2.2) * (rand() < 0.8 ? 1 : -1)
      this.radius[i] = 0.32 + rand() * 0.42
      this.rise[i] = -0.15 + rand() * 0.45
      this.tumble[i] = 2 + rand() * 6
    }
    this.age = 0
    this.driftX = this.driftY = this.driftZ = 0
  }

  /** `wind` (optional, `vrm-wind.ts` units) carries the swarm downwind as it flies. */
  step(dt: number, wind?: Readonly<Point3>): void {
    if (this.age < 0) return
    const step = Number.isFinite(dt) ? Math.min(Math.max(0, dt), 0.1) : 0
    this.age += step
    if (wind) {
      this.driftX += wind.x * PETAL_WIND_DRIFT * step
      this.driftY += wind.y * PETAL_WIND_DRIFT * step
      this.driftZ += wind.z * PETAL_WIND_DRIFT * step
    }
    const age = this.age
    if (age >= PETAL_LIFE_SEC) {
      this.age = -1
      this.count = 0
      return
    }
    const size = smooth(age / GROW_SEC) * clamp01((PETAL_LIFE_SEC - age) / 0.5)
    const gather = smooth(age / GATHER_SEC)
    const outward = smooth((age - (PETAL_LIFE_SEC - SCATTER_SEC)) / SCATTER_SEC)
    for (let i = 0; i < this.count; i++) this.place(i, age, gather, outward, size)
  }

  private place(i: number, age: number, gather: number, outward: number, size: number): void {
    const angle = this.angle[i] + this.spin[i] * age
    const radius = this.radius[i] * (1 + outward * 1.8)
    const orbitX = this.centerX + Math.cos(angle) * radius
    const orbitY = this.startY[i] + this.rise[i] * age + Math.sin(angle * 2) * 0.05 + outward * 0.25
    const orbitZ = this.centerZ + Math.sin(angle) * radius * 0.8 + 0.1
    this.x[i] = this.startX[i] + (orbitX - this.startX[i]) * gather + this.driftX
    this.y[i] = this.startY[i] + (orbitY - this.startY[i]) * gather + this.driftY
    this.z[i] = this.startZ[i] + (orbitZ - this.startZ[i]) * gather + this.driftZ
    this.rx[i] = age * this.tumble[i]
    this.ry[i] = angle
    this.rz[i] = age * this.tumble[i] * 0.6
    this.scale[i] = size
  }
}
