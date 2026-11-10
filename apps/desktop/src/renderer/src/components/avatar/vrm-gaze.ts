import { type SpringState, springTo } from './damped-spring'

/** Olhar do VRM: a cabeça acompanha o alvo e o olho faz sacadas pequenas em volta dele. */

const DEG = Math.PI / 180
export const HEAD_LIMITS = { yaw: 40 * DEG, up: 10 * DEG, down: 22 * DEG }
/** Cap on the head bone's final back tilt: pose, idle, sips and gaze add up on it (26.5° seen live). */
export const HEAD_BACK_LIMIT = 8 * DEG
/** Parte do ângulo que a cabeça assume; o olho cobre o resto, como numa pessoa. */
export const HEAD_FOLLOW = 0.55
/** Divisão entre pescoço e cabeça do quanto ela gira. */
export const NECK_SHARE = 0.35
/** Rigidez da mola da cabeça: ~0.6 s até 95%, o mesmo tempo da antiga exponencial, sem o tranco inicial. */
const HEAD_OMEGA = 8
const SACCADE_RATE = 35
/** Deslocamento da sacada em metros, no plano do alvo (~2 m à frente). */
const SACCADE_REACH = 0.12
/** VRoid mapeia 90° de olhar em 10° de olho: o olho nunca saía do centro. */
export const EYE_INPUT_MAX_DEG = 25

export interface Vec3Like {
  x: number
  y: number
  z: number
}

export interface HeadAngles {
  /** Rotação Y do osso normalizado: positivo gira para +X. */
  yaw: number
  /** Rotação X do osso normalizado: positivo inclina para baixo. */
  pitch: number
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

/** Ângulo até o alvo, já na parcela e nos limites da cabeça. */
export function headAnglesToward(target: Vec3Like, origin: Vec3Like): HeadAngles {
  const dx = target.x - origin.x
  const dy = target.y - origin.y
  const dz = Math.max(0.05, target.z - origin.z)
  const yaw = Math.atan2(dx, dz) * HEAD_FOLLOW
  const pitch = -Math.atan2(dy, Math.hypot(dx, dz)) * HEAD_FOLLOW
  return {
    yaw: clamp(yaw, -HEAD_LIMITS.yaw, HEAD_LIMITS.yaw),
    pitch: clamp(pitch, -HEAD_LIMITS.up, HEAD_LIMITS.down),
  }
}

interface RangeMapLike {
  inputMaxValue: number
}

/** Mantém o giro máximo do olho do modelo, mas atinge-o com um ângulo de olhar menor. */
export function tightenEyeRange(applier: object | null | undefined): void {
  if (!applier) return
  const maps = applier as Partial<Record<string, RangeMapLike>>
  for (const key of [
    'rangeMapHorizontalInner',
    'rangeMapHorizontalOuter',
    'rangeMapVerticalDown',
    'rangeMapVerticalUp',
  ]) {
    const map = maps[key]
    if (map && map.inputMaxValue > EYE_INPUT_MAX_DEG) map.inputMaxValue = EYE_INPUT_MAX_DEG
  }
}

export class GazeDriver {
  readonly head: HeadAngles = { yaw: 0, pitch: 0 }
  readonly saccade = { x: 0, y: 0 }
  private saccadeGoal = { x: 0, y: 0 }
  private nextSaccadeIn: number
  private readonly yaw: SpringState = { value: 0, velocity: 0 }
  private readonly pitch: SpringState = { value: 0, velocity: 0 }

  constructor(private readonly random: () => number = Math.random) {
    this.nextSaccadeIn = this.saccadeInterval()
  }

  /** Velocidade angular da cabeça (rad/s): a piscada usa para acompanhar giros largos. */
  get speed(): number {
    return Math.hypot(this.yaw.velocity, this.pitch.velocity)
  }

  /** Aproxima a cabeça do alvo com amortecimento e sorteia a próxima sacada quando vence. */
  advance(dt: number, desired: HeadAngles): void {
    this.head.yaw = springTo(this.yaw, desired.yaw, HEAD_OMEGA, dt)
    this.head.pitch = springTo(this.pitch, desired.pitch, HEAD_OMEGA, dt)
    this.nextSaccadeIn -= dt
    if (this.nextSaccadeIn <= 0) {
      // Metade das vezes o olho volta ao centro: fixação com desvios, não um olhar à deriva.
      const centered = this.random() < 0.5
      this.saccadeGoal = centered
        ? { x: 0, y: 0 }
        : { x: (this.random() * 2 - 1) * SACCADE_REACH, y: (this.random() * 2 - 1) * SACCADE_REACH * 0.5 }
      this.nextSaccadeIn = this.saccadeInterval()
    }
    const jump = 1 - Math.exp(-dt * SACCADE_RATE)
    this.saccade.x += (this.saccadeGoal.x - this.saccade.x) * jump
    this.saccade.y += (this.saccadeGoal.y - this.saccade.y) * jump
  }

  private saccadeInterval(): number {
    return 0.6 + this.random() * 1.8
  }
}
