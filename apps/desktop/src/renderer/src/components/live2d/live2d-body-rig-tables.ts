import type { ParameterRange } from './live2d-animator-parameter-probe'
import type { Spring } from './live2d-physics'

export const JOINT_CANDIDATES = {
  torso: ['ParamBodyUpper'],

  shoulder: ['ParamShoulder'],
  shoulderL: ['ParamLeftShoulderUp', 'ParamShoulderL'],
  shoulderR: ['ParamRightShoulderUp', 'ParamShoulderR'],

  ahoge: ['ParamHairAhoge'],
  hairFront: ['ParamHairFront'],
  hairSide: ['ParamHairSide', 'ParamHairSide2'],
  hairBack: ['ParamHairBack'],

  upperArmL: ['ParamArmLA01', 'ParamArmLB01', 'ParamArmL01', 'ParamArmL'],
  upperArmR: ['ParamArmRA01', 'ParamArmRB01', 'ParamArmR01', 'ParamArmR'],
  foreArmL: ['ParamArmLA02', 'ParamArmLB02', 'ParamArmL02'],
  foreArmR: ['ParamArmRA02', 'ParamArmRB02', 'ParamArmR02'],
  handL: ['ParamArmLA03', 'ParamArmLB03', 'ParamArmL03', 'ParamHandLB', 'ParamHandAngleL'],
  handR: ['ParamArmRA03', 'ParamArmRB03', 'ParamArmR03', 'ParamHandRB', 'ParamHandAngleR'],

  legL: ['ParamLeftLeg'],
  legR: ['ParamRightLeg'],
  knee: ['ParamLegKnee'],
} as const

export type JointName = keyof typeof JOINT_CANDIDATES

export const JOINT_GAIN: Record<JointName, number> = {
  torso: 0.12,
  shoulder: 0.16,
  shoulderL: 0.16,
  shoulderR: 0.16,

  ahoge: 0.5,
  hairFront: 0.35,
  hairSide: 0.5,
  hairBack: 0.45,
  upperArmL: 0.1,
  upperArmR: 0.1,
  foreArmL: 0.16,
  foreArmR: 0.16,
  handL: 0.24,
  handR: 0.24,
  legL: 0.14,
  legR: 0.14,
  knee: 0.2,
}

export const JOINT_SPRING: Record<JointName, readonly [stiffness: number, damping: number, mass: number]> = {
  torso: [16, 5.5, 1.6],
  shoulder: [26, 5, 1],
  shoulderL: [26, 5, 1],
  shoulderR: [26, 5, 1],

  ahoge: [7, 2.2, 2],
  hairFront: [8, 2.4, 1.6],
  hairSide: [6, 2, 1.8],
  hairBack: [5, 2, 2],
  upperArmL: [18, 4.2, 1],
  upperArmR: [18, 4.2, 1],
  foreArmL: [13, 3.4, 1.4],
  foreArmR: [13, 3.4, 1.4],
  handL: [9, 2.8, 1.8],
  handR: [9, 2.8, 1.8],
  legL: [14, 5, 1],
  legR: [14, 5, 1],
  knee: [12, 4.5, 1],
}

export interface ResolvedJoint {
  readonly name: JointName

  readonly params: readonly ParameterRange[]
  readonly spring: Spring
}

export interface BodyPosture {
  readonly energy: number

  readonly lift: number

  readonly shoulderTension: number
}

const NEUTRAL_POSTURE: BodyPosture = { energy: 1, lift: 0, shoulderTension: 0 }

const POSTURE_BY_EXPRESSION: Record<string, BodyPosture> = {
  excited: { energy: 1.6, lift: 0.45, shoulderTension: 0.3 },
  happy: { energy: 1.35, lift: 0.3, shoulderTension: 0.15 },
  laugh: { energy: 1.5, lift: 0.35, shoulderTension: 0.2 },
  love: { energy: 1.1, lift: 0.25, shoulderTension: 0.1 },
  panic: { energy: 1.8, lift: 0.5, shoulderTension: 0.85 },
  scared: { energy: 1.5, lift: 0.4, shoulderTension: 1 },
  surprised: { energy: 1.3, lift: 0.35, shoulderTension: 0.6 },
  angry: { energy: 1.2, lift: -0.1, shoulderTension: 0.55 },
  determined: { energy: 1.1, lift: 0.1, shoulderTension: 0.4 },
  evil: { energy: 0.9, lift: -0.05, shoulderTension: 0.3 },
  thinking: { energy: 0.75, lift: 0.05, shoulderTension: -0.1 },
  shy: { energy: 0.7, lift: 0.3, shoulderTension: 0.45 },
  embarrassed: { energy: 0.7, lift: 0.35, shoulderTension: 0.5 },
  sad: { energy: 0.55, lift: -0.35, shoulderTension: -0.55 },
  bored: { energy: 0.5, lift: -0.3, shoulderTension: -0.45 },
  tired: { energy: 0.45, lift: -0.45, shoulderTension: -0.7 },
  sleep: { energy: 0.25, lift: -0.5, shoulderTension: -0.8 },
}

export function getPostureForExpression(expression: string): BodyPosture {
  return POSTURE_BY_EXPRESSION[expression] ?? NEUTRAL_POSTURE
}

export interface BodyRigFrame {
  readonly dt: number
  readonly time: number

  readonly bodySwayVelocity: number

  readonly bodyHeaveVelocity: number

  readonly headSwayVelocity: number

  readonly breath: number
  readonly posture: BodyPosture

  readonly shiver: number

  readonly speechEnergy: number

  /** Sideways wind, -1..1 (+ blows toward the model's left), and its gust strength, 0..1. */
  readonly windSide: number
  readonly windGust: number
}
