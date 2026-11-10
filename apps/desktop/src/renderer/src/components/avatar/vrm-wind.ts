/**
 * Wind on the VRM's spring bones (hair, skirt, ribbons): no new physics, the wind is added to each
 * joint's authored gravity. The field is layered value noise — slow gusts and lulls, a direction
 * that wanders over minutes, a light flutter — sampled into a caller-owned vector (no allocation).
 */

export interface Vec3 {
  x: number
  y: number
  z: number
}

export interface SpringJointLike {
  bone?: { parent: unknown } | null
  settings: { gravityPower: number; gravityDir: Vec3 & { set(x: number, y: number, z: number): unknown } }
}

/** Gravity-units push at strength 1: four times Haru's authored gravity (0.025) — a stormy day. */
export const WIND_MAX = 0.1
/** Default strength (0..1): a breeze that sways the hair without blowing the skirt up. */
export const DEFAULT_WIND_STRENGTH = 0.35

const hash = (i: number) => {
  const s = Math.sin(i * 127.1 + 311.7) * 43758.5453
  return s - Math.floor(s)
}

/** Smooth 1-D value noise in [0, 1]: quintic interpolation between hashed lattice values. */
function noise(t: number): number {
  const i = Math.floor(t)
  const f = t - i
  const u = f * f * f * (f * (f * 6 - 15) + 10)
  return hash(i) + (hash(i + 1) - hash(i)) * u
}

/** Three octaves, normalized back to [0, 1]. */
function layered(t: number, seed: number): number {
  return (noise(t + seed) * 4 + noise(t * 2.13 + seed * 1.7) * 2 + noise(t * 4.37 + seed * 2.9)) / 7
}

const clamp01 = (value: number) => (value > 0 ? Math.min(value, 1) : 0)

/**
 * Wind at `t` seconds, scaled by `strength` (0..1), into `out`. It blows from behind the camera
 * (−Z, so strands stream back instead of into the body), the direction wandering ±~50° over minutes.
 */
export function windInto(out: Vec3, t: number, strength: number): Vec3 {
  const s = Number.isFinite(strength) ? clamp01(strength) : 0
  if (s === 0 || !Number.isFinite(t)) {
    out.x = out.y = out.z = 0
    return out
  }
  const yaw = (layered(t * 0.012, 3.1) - 0.5) * 2.4
  const gust = clamp01((layered(t * 0.09, 7.3) - 0.3) / 0.45) ** 2
  const flutter = (layered(t * 0.55, 1.9) - 0.5) * 0.3
  const power = s * WIND_MAX * (0.3 + 0.55 * gust) * (1 + flutter)
  out.x = Math.sin(yaw) * power
  out.z = -Math.cos(yaw) * power
  out.y = (layered(t * 0.4, 5.5) - 0.5) * power * 0.3
  return out
}

interface Captured {
  joint: SpringJointLike
  power: number
  dir: Vec3
  /** How much of the wind this joint catches: long chains and their tips catch the most. */
  share: number
}

/** Depth of each joint inside its spring chain, and that chain's length, from the bone hierarchy. */
function chainShares(joints: readonly SpringJointLike[]): number[] {
  const byBone = new Map<unknown, number>()
  joints.forEach((joint, i) => {
    if (joint.bone) byBone.set(joint.bone, i)
  })
  const depth = joints.map(() => 0)
  const root = joints.map((_, i) => i)
  joints.forEach((joint, i) => {
    let parent = joint.bone?.parent
    while (parent && byBone.has(parent)) {
      depth[i]++
      root[i] = byBone.get(parent) as number
      parent = (joints[root[i]].bone as { parent: unknown }).parent
    }
  })
  const longest = new Map<number, number>()
  for (let i = 0; i < joints.length; i++) longest.set(root[i], Math.max(longest.get(root[i]) ?? 0, depth[i] + 1))
  return joints.map((_, i) => {
    const length = longest.get(root[i]) ?? 1
    return Math.min(1, length / 5) * (0.55 + (0.45 * (depth[i] + 1)) / length)
  })
}

export class SpringWind {
  private readonly captured: Captured[]

  constructor(joints: Iterable<SpringJointLike>) {
    const list = [...joints]
    const shares = chainShares(list)
    this.captured = list.map((joint, i) => ({
      joint,
      power: joint.settings.gravityPower,
      dir: { x: joint.settings.gravityDir.x, y: joint.settings.gravityDir.y, z: joint.settings.gravityDir.z },
      share: shares[i],
    }))
  }

  apply(wind: Vec3): void {
    for (const { joint, power, dir, share } of this.captured) {
      const x = dir.x * power + wind.x * share
      const y = dir.y * power + wind.y * share
      const z = dir.z * power + wind.z * share
      const length = Math.hypot(x, y, z)
      if (length < 1e-6) {
        joint.settings.gravityDir.set(dir.x, dir.y, dir.z)
        joint.settings.gravityPower = 0
        continue
      }
      joint.settings.gravityDir.set(x / length, y / length, z / length)
      joint.settings.gravityPower = length
    }
  }

  restore(): void {
    for (const { joint, power, dir } of this.captured) {
      joint.settings.gravityDir.set(dir.x, dir.y, dir.z)
      joint.settings.gravityPower = power
    }
  }
}

/**
 * `character.windEnabled` + `character.windStrength` (0..100, schema in `config/schemas/character.rs`)
 * as the 0..1 strength both avatar backends take. Reduced motion stills the air: wind is decoration.
 */
export function windStrengthFromConfig(enabled: unknown, strength: unknown, reducedMotion: boolean): number {
  if (enabled === false || reducedMotion) return 0
  if (typeof strength !== 'number' || !Number.isFinite(strength)) return DEFAULT_WIND_STRENGTH
  return clamp01(strength / 100)
}
