import type { VRM, VRMHumanBoneName } from '@pixiv/three-vrm'
import type * as THREE from 'three'
import { ARM_REST, type ArmPose } from './vrm-body-rig'
import { type GazeDriver, headAnglesToward, NECK_SHARE } from './vrm-gaze'

type Humanoid = VRM['humanoid']

/**
 * Every arm bone is written on all three axes: IK (the katana grip) and clips leave whole
 * quaternions behind, and an axis nobody resets would carry them into every later frame.
 */
export function writeArmPose(vrm: VRM, pose: ArmPose): void {
  const humanoid = vrm.humanoid
  if (!humanoid) return
  humanoid
    .getNormalizedBoneNode('leftUpperArm')
    ?.rotation.set(pose.leftUpperArmX, pose.leftUpperArmY, pose.leftUpperArm)
  humanoid
    .getNormalizedBoneNode('rightUpperArm')
    ?.rotation.set(pose.rightUpperArmX, pose.rightUpperArmY, pose.rightUpperArm)
  humanoid
    .getNormalizedBoneNode('leftLowerArm')
    ?.rotation.set(pose.leftLowerArmX, pose.leftLowerArmY, pose.leftLowerArm)
  humanoid
    .getNormalizedBoneNode('rightLowerArm')
    ?.rotation.set(pose.rightLowerArmX, pose.rightLowerArmY, pose.rightLowerArm)
  humanoid.getNormalizedBoneNode('leftHand')?.rotation.set(pose.leftHandX, pose.leftHandY, pose.leftHandZ)
  humanoid.getNormalizedBoneNode('rightHand')?.rotation.set(pose.rightHandX, pose.rightHandY, pose.rightHandZ)
  applyHandPose(vrm, 'left', pose.leftGrip, pose.leftPeace, pose.heart, 0)
  applyHandPose(vrm, 'right', pose.rightGrip, pose.rightPeace, pose.heart, pose.rightThumb)
}

const FINGER_WEIGHT: Record<string, number> = { Index: 0.84, Middle: 0.96, Ring: 1.1, Little: 1.24 }

function applyHandPose(
  vrm: VRM,
  side: 'left' | 'right',
  grip: number,
  peace: number,
  heart: number,
  thumbUp: number,
): void {
  const sign = side === 'left' ? -1 : 1
  for (const finger of ['Index', 'Middle', 'Ring', 'Little'] as const) {
    const extended = finger === 'Index' || finger === 'Middle'
    const cascade = peace > 0 ? 1 : FINGER_WEIGHT[finger]
    const curl = grip * cascade * (extended ? 1 - peace : 1)
    const spread = extended ? peace * (finger === 'Index' ? 0.12 : -0.12) : 0
    applyFingerPose(vrm, `${side}${finger}`, sign * curl, spread)
  }
  const thumb = vrm.humanoid.getNormalizedBoneNode(`${side}ThumbProximal`)
  if (peace > 0) {
    vrm.humanoid.getNormalizedBoneNode(`${side}ThumbMetacarpal`)?.rotation.set(0, -sign * 0.35, sign * 0.5)
    thumb?.rotation.set(0, -sign * 0.3, sign * 0.55)
    vrm.humanoid.getNormalizedBoneNode(`${side}ThumbDistal`)?.rotation.set(0, 0, sign * 0.45)
    return
  }
  // Thumbs-up: the thumb stays straight out of the fist instead of folding over it.
  if (thumb) thumb.rotation.set(0, sign * grip * 0.8 * (1 - thumbUp), 0)
  // The hilt grip curls this joint too; nothing else does, so it rests straight.
  vrm.humanoid.getNormalizedBoneNode(`${side}ThumbDistal`)?.rotation.set(0, 0, 0)
  const thumbBase = vrm.humanoid.getNormalizedBoneNode(`${side}ThumbMetacarpal`)
  thumbBase?.rotation.set(0, sign * thumbUp * 0.35, -sign * (heart * 1.1 + grip * 0.25 * (1 - peace) * (1 - thumbUp)))
}

function applyFingerPose(vrm: VRM, finger: string, curl: number, spread: number): void {
  for (const joint of ['Proximal', 'Intermediate', 'Distal'] as const) {
    const node = vrm.humanoid.getNormalizedBoneNode(`${finger}${joint}` as VRMHumanBoneName)
    if (!node) continue
    const flexion = joint === 'Proximal' ? 1.1 : joint === 'Intermediate' ? 1.5 : 0.9
    node.rotation.set(0, joint === 'Proximal' ? spread : 0, curl * flexion)
  }
}

function applyLegAndHipOffsets(humanoid: Humanoid, bodyPose: ArmPose): void {
  if (bodyPose.leftUpperLegX !== undefined) {
    const node = humanoid.getNormalizedBoneNode('leftUpperLeg')
    if (node) node.rotation.x = bodyPose.leftUpperLegX
  }
  if (bodyPose.rightUpperLegX !== undefined) {
    const node = humanoid.getNormalizedBoneNode('rightUpperLeg')
    if (node) node.rotation.x = bodyPose.rightUpperLegX
  }
  if (bodyPose.leftLowerLegX !== undefined) {
    const node = humanoid.getNormalizedBoneNode('leftLowerLeg')
    if (node) node.rotation.x = bodyPose.leftLowerLegX
  }
  if (bodyPose.rightLowerLegX !== undefined) {
    const node = humanoid.getNormalizedBoneNode('rightLowerLeg')
    if (node) node.rotation.x = bodyPose.rightLowerLegX
  }
}

export function addBodyPoseOffsets(humanoid: Humanoid, bodyPose: ArmPose): void {
  const chest = humanoid.getNormalizedBoneNode('chest')
  const spine = humanoid.getNormalizedBoneNode('spine')
  const head = humanoid.getNormalizedBoneNode('head')
  if (chest) chest.rotation.x += bodyPose.chestX
  if (spine) spine.rotation.z += bodyPose.spineZ
  if (head) {
    head.rotation.x += bodyPose.headX
    head.rotation.y += bodyPose.headY
    head.rotation.z += bodyPose.headZ
  }
  applyLegAndHipOffsets(humanoid, bodyPose)
}

export function readArmPose(humanoid: Humanoid | undefined, bodyPose: ArmPose): ArmPose {
  if (!humanoid) return { ...ARM_REST }
  const read = (bone: 'leftUpperArm' | 'rightUpperArm' | 'leftLowerArm' | 'rightLowerArm') =>
    humanoid.getNormalizedBoneNode(bone)?.rotation.z ?? ARM_REST[bone]
  return {
    ...bodyPose,
    leftUpperArm: read('leftUpperArm'),
    rightUpperArm: read('rightUpperArm'),
    leftLowerArm: read('leftLowerArm'),
    rightLowerArm: read('rightLowerArm'),
    leftLowerArmY: humanoid.getNormalizedBoneNode('leftLowerArm')?.rotation.y ?? 0,
    rightLowerArmY: humanoid.getNormalizedBoneNode('rightLowerArm')?.rotation.y ?? 0,
    leftUpperArmX: humanoid.getNormalizedBoneNode('leftUpperArm')?.rotation.x ?? 0,
    leftUpperArmY: humanoid.getNormalizedBoneNode('leftUpperArm')?.rotation.y ?? 0,
    rightUpperArmX: humanoid.getNormalizedBoneNode('rightUpperArm')?.rotation.x ?? 0,
    rightUpperArmY: humanoid.getNormalizedBoneNode('rightUpperArm')?.rotation.y ?? 0,
    leftLowerArmX: humanoid.getNormalizedBoneNode('leftLowerArm')?.rotation.x ?? 0,
    rightLowerArmX: humanoid.getNormalizedBoneNode('rightLowerArm')?.rotation.x ?? 0,
    leftHandY: humanoid.getNormalizedBoneNode('leftHand')?.rotation.y ?? 0,
    rightHandY: humanoid.getNormalizedBoneNode('rightHand')?.rotation.y ?? 0,
  }
}

export function applyIdleSway(humanoid: Humanoid, breath: number, weight: number): void {
  const chest = humanoid.getNormalizedBoneNode('chest')
  if (chest) {
    chest.rotation.x = breath * 0.02
    chest.rotation.y = weight * 0.008
    chest.rotation.z = weight * 0.012
  }
  const spine = humanoid.getNormalizedBoneNode('spine')
  if (spine) {
    spine.rotation.x = breath * 0.008
    spine.rotation.y = -weight * 0.005
    spine.rotation.z = weight * 0.018
  }
  // Contrapposto: weight on one leg tilts the pelvis, the legs undo it so the feet stay planted.
  const hips = humanoid.getNormalizedBoneNode('hips')
  if (hips) {
    hips.rotation.z = CONTRAPPOSTO_HIP - weight * 0.012
    // Absolute base for the ambient twist added later in the frame, or it accumulates every frame.
    hips.rotation.y = 0
  }

  const leftShoulder = humanoid.getNormalizedBoneNode('leftShoulder')
  const rightShoulder = humanoid.getNormalizedBoneNode('rightShoulder')
  if (leftShoulder) {
    leftShoulder.rotation.z = -breath * 0.012 + weight * 0.008
    leftShoulder.rotation.y = -breath * 0.006
  }
  if (rightShoulder) {
    rightShoulder.rotation.z = breath * 0.012 + weight * 0.008
    rightShoulder.rotation.y = breath * 0.006
  }

  const leftUpperLeg = humanoid.getNormalizedBoneNode('leftUpperLeg')
  const rightUpperLeg = humanoid.getNormalizedBoneNode('rightUpperLeg')
  // Anime stance: legs drawn together, thighs and toes turned slightly in (uchimata), the free
  // right knee leaning toward the standing leg. Parallel, apart feet read as standing at attention.
  if (leftUpperLeg) {
    leftUpperLeg.rotation.z = weight * 0.01 - CONTRAPPOSTO_HIP - LEG_ADDUCT
    leftUpperLeg.rotation.y = -TOE_IN
  }
  if (rightUpperLeg) {
    rightUpperLeg.rotation.z = weight * 0.01 - CONTRAPPOSTO_HIP + LEG_ADDUCT * 1.6
    rightUpperLeg.rotation.y = TOE_IN * 1.4
  }
  // Knees meet while the feet stay a little apart: the soft "A" of an anime girl standing.
  const leftLowerLeg = humanoid.getNormalizedBoneNode('leftLowerLeg')
  const rightLowerLeg = humanoid.getNormalizedBoneNode('rightLowerLeg')
  if (leftLowerLeg) leftLowerLeg.rotation.z = KNEE_IN
  if (rightLowerLeg) rightLowerLeg.rotation.z = -KNEE_IN * 1.3
}

const CONTRAPPOSTO_HIP = 0.05
const LEG_ADDUCT = 0.05
const KNEE_IN = 0.06
const TOE_IN = 0.1

export function applyIdleArmSway(humanoid: Humanoid | undefined, breath: number, sway: number): void {
  if (!humanoid) return
  const leftUpper = humanoid.getNormalizedBoneNode('leftUpperArm')
  const rightUpper = humanoid.getNormalizedBoneNode('rightUpperArm')
  if (leftUpper) {
    leftUpper.rotation.z -= breath * 0.015 - sway * 0.012
    leftUpper.rotation.x -= sway * 0.015
  }
  if (rightUpper) {
    rightUpper.rotation.z += breath * 0.015 + sway * 0.012
    rightUpper.rotation.x -= sway * 0.015
  }
  const leftLower = humanoid.getNormalizedBoneNode('leftLowerArm')
  const rightLower = humanoid.getNormalizedBoneNode('rightLowerArm')
  if (leftLower) leftLower.rotation.y -= breath * 0.02
  if (rightLower) rightLower.rotation.y += breath * 0.02
  const leftHand = humanoid.getNormalizedBoneNode('leftHand')
  const rightHand = humanoid.getNormalizedBoneNode('rightHand')
  if (leftHand) {
    leftHand.rotation.z += sway * 0.015
    leftHand.rotation.x += breath * 0.012
  }
  if (rightHand) {
    rightHand.rotation.z += sway * 0.015
    rightHand.rotation.x += breath * 0.012
  }
}

export function applyGaze(
  vrm: VRM,
  gaze: GazeDriver,
  lookTarget: THREE.Object3D,
  eyeTarget: THREE.Object3D,
  headOrigin: THREE.Vector3,
  busy: boolean,
  dt: number,
): void {
  const head = vrm.humanoid.getNormalizedBoneNode('head')
  const neck = vrm.humanoid.getNormalizedBoneNode('neck')
  if (head) head.getWorldPosition(headOrigin)
  gaze.advance(dt, busy ? { yaw: 0, pitch: 0 } : headAnglesToward(lookTarget.position, headOrigin))
  const { yaw, pitch } = gaze.head
  if (neck) neck.rotation.set(pitch * NECK_SHARE, yaw * NECK_SHARE, 0)
  if (head) {
    head.rotation.x += pitch * (1 - NECK_SHARE)
    head.rotation.y += yaw * (1 - NECK_SHARE)
  }
  eyeTarget.position.set(
    lookTarget.position.x + gaze.saccade.x,
    lookTarget.position.y + gaze.saccade.y,
    lookTarget.position.z,
  )
}

export function animateVrmHead(
  head: THREE.Object3D,
  nodImpulse: number,
  breath: number,
  weightLag: number,
  actionRemaining: number,
  actionDuration: number,
  actionName: string,
): void {
  const nodRot = Math.sin(nodImpulse * Math.PI * 2) * 0.25 * nodImpulse
  head.rotation.x = nodRot + breath * 0.006
  head.rotation.y = nodImpulse > 0 ? 0 : weightLag * 0.03
  const envelope = actionRemaining > 0 ? Math.sin((actionRemaining / actionDuration) * Math.PI) ** 2 : 0
  head.rotation.z = weightLag * 0.012
  if (actionName === 'shakehead') head.rotation.y += Math.sin(actionRemaining * 14) * 0.22 * envelope
}

function applySpeechTorso(humanoid: Humanoid, intensity: number, elapsed: number): void {
  const head = humanoid.getNormalizedBoneNode('head')
  if (head) {
    head.rotation.x += (Math.sin(elapsed * 3.2) * 0.02 + Math.sin(elapsed * 4.8) * 0.015) * intensity
    head.rotation.z += Math.sin(elapsed * 2.1) * 0.012 * intensity
  }
  const chest = humanoid.getNormalizedBoneNode('chest')
  if (chest) chest.rotation.x += Math.sin(elapsed * 3.2) * 0.012 * intensity
  const leftShoulder = humanoid.getNormalizedBoneNode('leftShoulder')
  const rightShoulder = humanoid.getNormalizedBoneNode('rightShoulder')
  if (leftShoulder) leftShoulder.rotation.z -= intensity * 0.008 * Math.sin(elapsed * 3.2)
  if (rightShoulder) rightShoulder.rotation.z += intensity * 0.008 * Math.sin(elapsed * 3.2)
}

function applySpeechRightArm(humanoid: Humanoid, factor: number, elapsed: number): void {
  const upper = humanoid.getNormalizedBoneNode('rightUpperArm')
  const lower = humanoid.getNormalizedBoneNode('rightLowerArm')
  const hand = humanoid.getNormalizedBoneNode('rightHand')
  if (upper) upper.rotation.x -= Math.sin(elapsed * 1.8) * 0.035 * factor
  if (lower) lower.rotation.y += Math.sin(elapsed * 2.4) * 0.045 * factor
  if (hand) hand.rotation.z += Math.sin(elapsed * 2.4) * 0.03 * factor
}

function applySpeechLeftArm(humanoid: Humanoid, factor: number, elapsed: number): void {
  const upper = humanoid.getNormalizedBoneNode('leftUpperArm')
  const lower = humanoid.getNormalizedBoneNode('leftLowerArm')
  const hand = humanoid.getNormalizedBoneNode('leftHand')
  if (upper) upper.rotation.x -= Math.sin(elapsed * 1.8 + 1) * 0.03 * factor
  if (lower) lower.rotation.y -= Math.sin(elapsed * 2.4 + 1) * 0.04 * factor
  if (hand) hand.rotation.z -= Math.sin(elapsed * 2.4 + 1) * 0.025 * factor
}

export function applySpeechGesture(
  humanoid: Humanoid | undefined,
  speaking: boolean,
  volume: number,
  elapsed: number,
): void {
  if (!humanoid || !speaking) return
  const intensity = Math.min(1, Math.max(0.15, volume * 1.5))
  applySpeechTorso(humanoid, intensity, elapsed)
  const bias = Math.sin(elapsed * 0.7)
  if (bias >= -0.3) applySpeechRightArm(humanoid, Math.max(0, bias + 0.3) * intensity, elapsed)
  if (bias <= 0.3) applySpeechLeftArm(humanoid, Math.max(0, 0.3 - bias) * intensity, elapsed)
}
