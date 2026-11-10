/**
 * Movimento secundário dos gestos novos, somado por cima da pose-alvo (`vrm-body-poses.ts`) já
 * envelopada. `amount` é o envelope da ação (0 na entrada e na saída); os trechos com fases próprias
 * (tranco do cochilo, soquinho) usam keyframes que já nascem e morrem em zero. Nada aqui aloca.
 */
import type { VrmBodyAction } from '@mika/sdk/contracts'
import type { ArmPose } from './vrm-body-rig'
import { hop, type Keyframes, sampleKeys, swell } from './vrm-keyframes'

export type Performance = (pose: ArmPose, progress: number, amount: number) => void

/** Fase rítmica que só começa depois da chegada à pose — mesma régua dos gestos antigos. */
const beatPhase = (progress: number) => Math.max(0, progress - 0.3) * Math.PI * 8

function addClap(pose: ArmPose, progress: number, amount: number): void {
  const open = hop(progress * Math.PI * 12) * amount
  pose.leftLowerArmY -= open * 0.36
  pose.rightLowerArmY += open * 0.36
  pose.headX += (1 - open) * 0.03 * amount
}

const THUMB_PUSH: Keyframes = [
  [0.22, 0],
  [0.34, 0.18],
  [0.46, 0],
]

function addThumbsUp(pose: ArmPose, progress: number): void {
  const push = sampleKeys(THUMB_PUSH, progress)
  pose.rightUpperArmX -= push
  pose.rightLowerArm -= push * 0.4
}

function addHeadshake(pose: ArmPose, progress: number, amount: number): void {
  const swing = Math.sin(beatPhase(progress) * 1.2) * amount * swell(progress)
  pose.headY += swing * 0.22
  pose.rightHandZ += swing * 0.28
  pose.rightLowerArmY -= swing * 0.12
}

function addBounce(pose: ArmPose, progress: number, amount: number): void {
  const lift = hop(progress * Math.PI * 10) * amount
  pose.leftUpperLegX -= lift * 0.2
  pose.rightUpperLegX -= lift * 0.2
  pose.leftLowerLegX += lift * 0.34
  pose.rightLowerLegX += lift * 0.34
  pose.hipsY -= lift * 0.05
  pose.leftUpperArm += lift * 0.1
  pose.rightUpperArm -= lift * 0.1
  pose.headX += lift * 0.05
}

function addSlump(pose: ArmPose, progress: number, amount: number): void {
  pose.chestX += Math.sin(progress * Math.PI * 2) * 0.03 * amount
  pose.headZ += Math.sin(progress * Math.PI * 1.5) * 0.04 * amount
}

function addFocus(pose: ArmPose, progress: number, envelope: number): void {
  const phase = beatPhase(progress)
  const amount = envelope * swell(progress)
  // Thumbs working the sticks, and a flinch when something happens on screen.
  pose.leftHandZ += Math.sin(phase * 3.1) * 0.06 * amount
  pose.rightHandZ += Math.sin(phase * 2.3 + 1) * 0.06 * amount
  pose.chestX += Math.max(0, Math.sin(phase * 0.7)) ** 4 * 0.05 * amount
}

function addGroove(pose: ArmPose, progress: number, amount: number): void {
  const beat = Math.sin(progress * Math.PI * 8) * amount
  const bob = hop(progress * Math.PI * 8) * amount
  pose.spineZ += beat * 0.09
  pose.headZ -= beat * 0.07
  pose.leftUpperArm += beat * 0.12
  pose.rightUpperArm += beat * 0.12
  pose.leftLowerArm -= bob * 0.2
  pose.rightLowerArm += bob * 0.2
  pose.leftUpperLegX -= bob * 0.08
  pose.rightUpperLegX -= bob * 0.08
  pose.leftLowerLegX += bob * 0.14
  pose.rightLowerLegX += bob * 0.14
  pose.hipsY -= bob * 0.022
}

// Sinks slowly, snaps awake, sinks again, then wakes for good.
const DOZE_HEAD: Keyframes = [
  [0.12, 0],
  [0.34, 0.42],
  [0.42, 0.46],
  [0.48, 0.02],
  [0.56, 0.05],
  [0.7, 0.38],
  [0.8, 0.42],
  [0.9, 0],
]

function addDoze(pose: ArmPose, progress: number, amount: number): void {
  const droop = sampleKeys(DOZE_HEAD, progress)
  pose.headX += droop
  pose.chestX += droop * 0.35
  pose.headZ += droop * 0.2
  pose.spineZ += Math.sin(progress * Math.PI * 3) * 0.04 * amount
}

// Two sharp pulls down — "yes! yes!" — back to the cocked fist in between.
const FIST_PUMP: Keyframes = [
  [0.3, 0],
  [0.38, 0],
  [0.45, 0.7],
  [0.53, 0.7],
  [0.6, 0.1],
  [0.67, 0.7],
  [0.75, 0.7],
  [0.84, 0],
]

function addFistPump(pose: ArmPose, progress: number): void {
  const pump = sampleKeys(FIST_PUMP, progress)
  pose.rightUpperArm += pump * 1.15
  pose.rightUpperArmX -= pump * 0.5
  pose.chestX += pump * 0.12
  pose.headX += pump * 0.08
}

// The startle: head and chest jerk back as the hands fly up, then a small shiver.
const FLINCH: Keyframes = [
  [0.1, 0],
  [0.2, 1],
  [0.4, 0.3],
  [0.7, 0.3],
  [0.85, 0],
]

function addGasp(pose: ArmPose, progress: number, amount: number): void {
  const flinch = sampleKeys(FLINCH, progress)
  pose.headX -= flinch * 0.1
  pose.chestX -= flinch * 0.06
  pose.headX += Math.sin(beatPhase(progress) * 2.4) * 0.012 * amount * swell(progress)
}

function addRage(pose: ArmPose, progress: number, amount: number): void {
  const shake = Math.sin(beatPhase(progress) * 2.6) * amount * swell(progress)
  pose.rightLowerArm += shake * 0.08
  pose.leftLowerArm += shake * 0.08
  pose.headX += Math.sin(beatPhase(progress) * 3.3 + 0.5) * 0.02 * amount * swell(progress)
  pose.chestX += shake * shake * 0.02
}

const TADA_SPRING: Keyframes = [
  [0.22, 0],
  [0.31, 0.14],
  [0.4, -0.03],
  [0.48, 0],
]

function addTada(pose: ArmPose, progress: number): void {
  const spring = sampleKeys(TADA_SPRING, progress)
  pose.leftUpperArm += spring
  pose.rightUpperArm -= spring
  pose.headX -= spring * 0.2
}

export const NEW_PERFORMANCES: Partial<Record<VrmBodyAction, Performance>> = {
  clap: addClap,
  'thumbs-up': addThumbsUp,
  headshake: addHeadshake,
  bounce: addBounce,
  slump: addSlump,
  focus: addFocus,
  groove: addGroove,
  doze: addDoze,
  'fist-pump': addFistPump,
  gasp: addGasp,
  rage: addRage,
  tada: addTada,
}
