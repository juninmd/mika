/** Coreografia VRM: preparação, pose legível, movimento secundário e acomodação. */
import { VRM_BODY_ACTIONS, type VrmBodyAction } from '@mika/sdk/contracts'

export { VRM_BODY_ACTIONS, type VrmBodyAction }

import { NEW_PERFORMANCES } from './vrm-body-moves'
import { POSES } from './vrm-body-poses'
import { easePose, hop, swell } from './vrm-keyframes'

export { easePose }

/** Radianos no humanoide normalizado; dedos usam flexão proporcional (0..1). */
export interface ArmPose {
  leftUpperArm: number
  rightUpperArm: number
  leftLowerArm: number
  rightLowerArm: number
  leftLowerArmY: number
  rightLowerArmY: number
  leftUpperArmX: number
  leftUpperArmY: number
  rightUpperArmX: number
  rightUpperArmY: number
  leftLowerArmX: number
  leftHandZ: number
  leftHandY: number
  rightLowerArmX: number
  rightHandZ: number
  rightHandY: number
  leftHandX: number
  rightHandX: number
  leftGrip: number
  rightGrip: number
  leftPeace: number
  rightPeace: number
  /** Polegar direito estendido sobre o punho fechado (joinha), 0..1. */
  rightThumb: number
  heart: number
  chestX: number
  spineZ: number
  headX: number
  headY: number
  headZ: number
  leftUpperLegX: number
  rightUpperLegX: number
  leftLowerLegX: number
  rightLowerLegX: number
  hipsY: number
}

export const ARM_REST: ArmPose = {
  leftUpperArm: -1.45,
  rightUpperArm: 1.47,
  leftLowerArm: 0.08,
  rightLowerArm: -0.12,
  leftLowerArmY: -0.38,
  rightLowerArmY: 0.42,
  leftUpperArmX: -0.26,
  leftUpperArmY: 0,
  rightUpperArmX: -0.28,
  rightUpperArmY: 0,
  leftLowerArmX: 0,
  leftHandZ: 0.18,
  leftHandY: 0,
  rightLowerArmX: 0,
  rightHandZ: -0.18,
  rightHandY: 0,
  leftHandX: 0.07,
  rightHandX: 0.09,
  leftGrip: 0.27,
  rightGrip: 0.29,
  leftPeace: 0,
  rightPeace: 0,
  rightThumb: 0,
  heart: 0,
  chestX: -0.015,
  spineZ: -0.13,
  headX: 0.005,
  headY: 0,
  headZ: 0.07,
  leftUpperLegX: 0,
  rightUpperLegX: 0,
  leftLowerLegX: 0,
  rightLowerLegX: 0.06,
  hipsY: 0,
}

export const BODY_ACTION_SECONDS: Readonly<Record<VrmBodyAction, number>> = {
  wave: 3.2,
  shrug: 2.6,
  cheer: 3,
  shy: 3.6,
  thinking: 4.2,
  laugh: 3.4,
  bow: 3.4,
  stretch: 4.6,
  presenting: 3.8,
  peace: 3,
  '22': 3.4,
  heart: 3.8,
  victory: 3.2,
  pout: 3.4,
  facepalm: 3.5,
  smug: 3.2,
  surprised: 2.8,
  sigh: 3.2,
  beckon: 2.8,
  curious: 3.2,
  salute: 2.8,
  whisper: 3.0,
  celebrate: 3.4,
  relaxed: 3.8,
  reading: 5.0,
  sitting: 6.0,
  point: 3.0,
  'six-seven': 4.8,
  floss: 4.8,
  'orange-justice': 5.2,
  yawn: 4,
  drink: 4.6,
  'throw-tomato': 3.2,
  senbonzakura: 5,
  clap: 3.4,
  'thumbs-up': 2.8,
  headshake: 3.2,
  bounce: 3,
  slump: 4,
  focus: 4.5,
  groove: 4.6,
  doze: 6.5,
  'fist-pump': 3.4,
  gasp: 3.6,
  rage: 3,
  tada: 3,
}

export function isVrmBodyAction(name: string): name is VrmBodyAction {
  return (VRM_BODY_ACTIONS as readonly string[]).includes(name)
}

const POSE_KEYS = Object.keys(ARM_REST) as (keyof ArmPose)[]

/** Escreve em `out` (sem alocar) a mistura de `from` para `to`. */
function blendInto(out: ArmPose, from: ArmPose, to: ArmPose, amount: number): void {
  for (const key of POSE_KEYS) out[key] = from[key] + (to[key] - from[key]) * amount
}

interface PoseChannel {
  key: keyof ArmPose
  offset: number
  delay: number
}

// Resolved once: the per-frame loop only walks these, no `Object.entries` or regex per frame.
const POSE_CHANNELS = {} as Record<VrmBodyAction, readonly PoseChannel[]>
for (const action of VRM_BODY_ACTIONS) {
  POSE_CHANNELS[action] = (Object.entries(POSES[action]) as [keyof ArmPose, number][]).map(([key, target]) => ({
    key,
    offset: target - ARM_REST[key],
    delay: /Lower|Hand|Grip|Peace|Thumb/.test(key) ? 0.04 : 0,
  }))
}

/** Envelope com tempo para ler a pose; antebraços e punhos chegam depois dos ombros. */
function envelope(progress: number, delay = 0): number {
  return easePose((progress - 0.09 - delay) / 0.22) * (1 - easePose((progress - 0.7 - delay) / (0.3 - delay)))
}

type DancePerformance = (pose: ArmPose, progress: number, amount: number) => void

function addSixSeven(pose: ArmPose, progress: number, amount: number): void {
  const beat = Math.sin(progress * Math.PI * 8) * amount
  const bounce = hop(progress * Math.PI * 8) * amount
  pose.leftUpperArm += beat * 0.72
  pose.rightUpperArm += beat * 0.72
  pose.leftLowerArm -= beat * 0.24
  pose.rightLowerArm -= beat * 0.24
  pose.spineZ -= beat * 0.09
  pose.headZ += beat * 0.055
  pose.leftUpperLegX -= bounce * 0.08
  pose.rightUpperLegX -= bounce * 0.08
  pose.leftLowerLegX += bounce * 0.12
  pose.rightLowerLegX += bounce * 0.12
  pose.hipsY -= bounce * 0.035
}

function addFloss(pose: ArmPose, progress: number, amount: number): void {
  const swing = Math.sin(progress * Math.PI * 10) * amount
  const bounce = hop(progress * Math.PI * 10) * amount
  pose.leftUpperArm += swing * 0.76
  pose.rightUpperArm += swing * 0.76
  pose.leftUpperArmX += swing * 0.3
  pose.rightUpperArmX += swing * 0.3
  pose.leftHandZ -= swing * 0.18
  pose.rightHandZ -= swing * 0.18
  pose.spineZ -= swing * 0.13
  pose.headZ += swing * 0.055
  pose.hipsY -= bounce * 0.025
}

function addOrangeJustice(pose: ArmPose, progress: number, amount: number): void {
  const cross = Math.sin(progress * Math.PI * 8) * amount
  // Meia onda suavizada (x^1.5): mesma fase e pausa do max(0, sin), sem a quina no zero.
  const lift = Math.max(0, Math.sin(progress * Math.PI * 4 - 0.65)) ** 1.5 * amount
  const bounce = (0.5 + Math.sin(progress * Math.PI * 8) * 0.5) * amount
  pose.leftUpperArm += cross * 0.52 + lift * 0.58
  pose.rightUpperArm += cross * 0.52 - lift * 0.58
  pose.leftLowerArm -= cross * 0.38
  pose.rightLowerArm -= cross * 0.38
  pose.leftHandZ -= cross * 0.16
  pose.rightHandZ -= cross * 0.16
  pose.spineZ -= cross * 0.11
  pose.leftUpperLegX -= bounce * 0.18
  pose.rightUpperLegX -= bounce * 0.18
  pose.leftLowerLegX += bounce * 0.26
  pose.rightLowerLegX += bounce * 0.26
  pose.hipsY -= bounce * 0.065
}

// Startle tremor: two close, non-multiple frequencies beat against each other instead of a clean
// sine, reading as a shaky flinch — the body counterpart of Live2D's `shiverVal`.
function addSurprisedTremor(pose: ArmPose, progress: number, amount: number): void {
  const phase = Math.max(0, progress - 0.3) * Math.PI * 8
  const tremor = amount * swell(progress)
  pose.headX += Math.sin(phase * 2.4) * 0.035 * tremor
  pose.headZ += Math.sin(phase * 3.1 + 0.6) * 0.03 * tremor
  pose.chestX += Math.sin(phase * 2.4) * 0.015 * tremor
}

/** Two sips: head and hand tip back together, then settle while she swallows. */
export function drinkSip(progress: number): number {
  // Meia onda ao quadrado: sobe e desce sem quina, e o gole termina parado.
  return Math.max(0, Math.sin((progress - 0.25) * Math.PI * 4)) ** 2 * envelope(progress)
}

function addDrinkSips(pose: ArmPose, progress: number): void {
  const sip = drinkSip(progress)
  pose.headX -= sip * 0.08
  pose.chestX -= sip * 0.05
  pose.rightLowerArm += sip * 0.15
  pose.rightHandX += sip * 0.4
}

// Wind-up behind the head, then the arm snaps forward past the release and follows through.
function addTomatoThrow(pose: ArmPose, progress: number): void {
  const windup = easePose((progress - 0.06) / 0.22) * (1 - easePose((progress - 0.28) / 0.2))
  pose.rightUpperArm -= windup * 2.3
  pose.rightUpperArmX += windup * 1.8
  pose.rightLowerArm -= windup * 1.5
  pose.rightGrip += windup * 0.8
  pose.chestX -= windup * 0.1
  pose.spineZ += windup * 0.12
  pose.headX -= windup * 0.06
}

// "Chire": once the blade has scattered, the free left hand sends the petals forward.
function addSenbonzakuraCommand(pose: ArmPose, progress: number, amount: number): void {
  const command = easePose((progress - 0.48) / 0.12) * amount
  pose.leftUpperArm += command * 1.25
  pose.leftUpperArmX -= command * 1.1
  pose.leftLowerArm -= command * 0.2
  pose.leftGrip -= command * 0.25
  pose.headY -= command * 0.12
}

const DANCE_PERFORMANCES: Partial<Record<VrmBodyAction, DancePerformance>> = {
  ...NEW_PERFORMANCES,
  drink: addDrinkSips,
  'throw-tomato': addTomatoThrow,
  senbonzakura: addSenbonzakuraCommand,
  'six-seven': addSixSeven,
  floss: addFloss,
  'orange-justice': addOrangeJustice,
  surprised: addSurprisedTremor,
}

function addPerformance(pose: ArmPose, action: VrmBodyAction, progress: number): void {
  const amount = envelope(progress)
  const phase = Math.max(0, progress - 0.3) * Math.PI * 8
  // O balanço rítmico entra devagar: o seno nasce com a velocidade máxima, e o punho arrancava.
  const sway = amount * swell(progress)
  DANCE_PERFORMANCES[action]?.(pose, progress, amount)
  if (action === 'wave') pose.rightHandZ += Math.sin(phase) * 0.3 * sway
  if (action === 'laugh') {
    pose.chestX += Math.sin(phase * 1.4) * 0.025 * sway
    pose.headX += Math.sin(phase * 1.4 - 0.45) * 0.04 * sway
  }
  if (action === 'cheer' || action === 'victory') pose.rightLowerArm += Math.sin(phase) * 0.15 * sway
  if (action === 'celebrate') {
    pose.leftUpperArm += Math.sin(phase) * 0.08 * sway
    pose.rightUpperArm -= Math.sin(phase) * 0.08 * sway
  }
  if (action === 'stretch') pose.spineZ += Math.sin(progress * Math.PI * 2) * 0.07 * amount
  if (action === 'thinking') pose.headY += Math.sin(progress * Math.PI * 2) * 0.06 * amount
  if (action === 'sigh') pose.chestX += Math.sin(progress * Math.PI) * 0.03 * amount
  if (action === 'beckon') pose.rightHandX += Math.sin(phase * 0.8) * 0.2 * sway
  if (action === 'pout') pose.headY += Math.sin(progress * Math.PI * 1.5) * 0.04 * amount
  if (action === 'smug') pose.spineZ += Math.sin(progress * Math.PI * 2) * 0.03 * amount
  if (action === 'curious') pose.headZ += Math.sin(progress * Math.PI * 1.5) * 0.04 * amount
  if (action === 'whisper') pose.chestX += Math.sin(progress * Math.PI) * 0.02 * amount
  if (action === 'reading') pose.headY += Math.sin(progress * Math.PI * 6) * 0.035 * amount
  if (action === 'sitting') pose.chestX += Math.sin(progress * Math.PI * 1.2) * 0.02 * amount
}

/** Mira do dedo indicador no cursor real — ganho angular por eixo, 0 fora da action 'point'. */
function addPointAim(pose: ArmPose, action: VrmBodyAction, progress: number, cursorX: number, cursorY: number): void {
  if (action !== 'point') return
  const amount = envelope(progress)
  const x = Math.max(-1, Math.min(1, cursorX))
  const y = Math.max(-1, Math.min(1, cursorY))
  pose.rightUpperArm += -x * 0.5 * amount
  pose.rightUpperArmX += y * 0.35 * amount
  pose.rightLowerArmY += y * 0.25 * amount
  pose.headZ -= x * 0.05 * amount
  pose.chestX -= x * 0.04 * amount
}

/** Escreve em `out` a pose da ação no `progress` (0..1); fora do intervalo, o repouso. */
export function armPoseInto(out: ArmPose, action: VrmBodyAction, progress: number, cursorX = 0, cursorY = 0): ArmPose {
  Object.assign(out, ARM_REST)
  if (!Number.isFinite(progress) || progress <= 0 || progress >= 1) return out
  for (const channel of POSE_CHANNELS[action]) out[channel.key] += channel.offset * envelope(progress, channel.delay)
  // Pequena inspiração antes do gesto, sem deslocar os pés ou usar escala no esqueleto.
  out.chestX -= Math.sin(Math.min(1, progress / 0.24) * Math.PI) ** 2 * 0.018
  addPerformance(out, action, progress)
  addPointAim(out, action, progress, cursorX, cursorY)
  return out
}

export function armPoseFor(action: VrmBodyAction, progress: number, cursorX = 0, cursorY = 0): ArmPose {
  return armPoseInto({ ...ARM_REST }, action, progress, cursorX, cursorY)
}

/** Toca uma ação por vez; os três buffers são reusados — nenhum objeto novo por quadro. */
export class VrmArmDriver {
  private action: VrmBodyAction | null = null
  private elapsed = 0
  private readonly current: ArmPose = { ...ARM_REST }
  private readonly target: ArmPose = { ...ARM_REST }
  private readonly transitionFrom: ArmPose = { ...ARM_REST }

  play(name: string): boolean {
    if (!isVrmBodyAction(name)) return false
    Object.assign(this.transitionFrom, this.current)
    this.action = name
    this.elapsed = 0
    return true
  }

  get playing(): boolean {
    return this.action !== null
  }

  /** Pose do quadro, ou null sem ação; o objeto devolvido é o buffer interno (só leitura). */
  advance(dt: number, cursorX = 0, cursorY = 0): Readonly<ArmPose> | null {
    const action = this.action
    if (!action) return null
    this.elapsed += Number.isFinite(dt) ? Math.max(0, dt) : 0
    const progress = this.elapsed / BODY_ACTION_SECONDS[action]
    if (progress >= 1) {
      this.action = null
      return Object.assign(this.current, ARM_REST)
    }
    armPoseInto(this.target, action, progress, cursorX, cursorY)
    blendInto(this.current, this.transitionFrom, this.target, easePose(this.elapsed / 0.24))
    return this.current
  }
}
