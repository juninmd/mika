import type { VRM, VRMHumanBoneName } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { type GripFrame, gripFrameInto, gripQuaternionInto, solveTwoBoneInto } from './vrm-grip'

/**
 * Puts the katana in her hands on the normalized rig, before `vrm.update()` copies it to the mesh:
 * the hilt rides the sword hand's palm frame (read from that rig's own bones), blends into a
 * two-handed guard before the face, and both arms reach it by two-bone IK with the hands turned so
 * the palms wrap it and the fingers curl closed. Every vector and quaternion is preallocated.
 */

type Side = 'left' | 'right'
const FINGERS = ['Index', 'Middle', 'Ring', 'Little'] as const
const JOINTS = ['Proximal', 'Intermediate', 'Distal'] as const
/** Flexion per joint that wraps a ~3 cm hilt: short of a fist, so the hilt fills the curl. */
const HILT_CURL = [1.05, 1.25, 0.75] as const
/** Outer fingers close tighter on a real grip; the index stays a touch looser. */
const FINGER_SHARE: Readonly<Record<(typeof FINGERS)[number], number>> = {
  Index: 0.9,
  Middle: 1,
  Ring: 1.06,
  Little: 1.12,
}
const THUMB_FLEX = 0.9
/** Where each palm closes on the hilt (sword-local Y; the hilt spans -0.12..0.12 under the tsuba). */
export const RIGHT_ON_HILT = 0.065
export const LEFT_ON_HILT = -0.05

/** Knuckles in hand-local space for rigs without finger bones (VRM T-pose: fingers along ±X, thumb +Z). */
const FALLBACK_KNUCKLE = { index: [0.085, 0.025], middle: [0.09, 0], little: [0.08, -0.03] } as const

interface Arm {
  side: 1 | -1
  upper?: THREE.Object3D
  lower?: THREE.Object3D
  hand: THREE.Object3D
  index?: THREE.Object3D
  middle?: THREE.Object3D
  little?: THREE.Object3D
  /** Finger joints paired with the flexion each takes at a full grip. */
  curls: { node: THREE.Object3D; flex: number }[]
  thumbBase?: THREE.Object3D
  thumb?: THREE.Object3D
  thumbTip?: THREE.Object3D
}

export interface HoldWeights {
  /** Sword present in the hand (0..1); fingers close with it. */
  hold: number
  /** 0 = hilt rides the sword hand where the arm put it, 1 = guard before the face. */
  raise: number
  /** Second hand on the hilt (0..1). */
  join: number
  /** 0 = held, 1 = let go and turned point-down. */
  release: number
}

const frame: GripFrame = { center: new THREE.Vector3(), axis: new THREE.Vector3(), normal: new THREE.Vector3() }
const wrist = new THREE.Vector3()
const knuckleI = new THREE.Vector3()
const knuckleM = new THREE.Vector3()
const knuckleL = new THREE.Vector3()
const rootPos = new THREE.Vector3()
const midPos = new THREE.Vector3()
const endPos = new THREE.Vector3()
const newMid = new THREE.Vector3()
const newEnd = new THREE.Vector3()
const pole = new THREE.Vector3()
const from = new THREE.Vector3()
const to = new THREE.Vector3()
const spot = new THREE.Vector3()
const axis = new THREE.Vector3()
const normal = new THREE.Vector3()
const up = new THREE.Vector3()
const out = new THREE.Vector3()
const qa = new THREE.Quaternion()
const qb = new THREE.Quaternion()
const qc = new THREE.Quaternion()
const qd = new THREE.Quaternion()
const kamaePos = new THREE.Vector3()
const kamaeQ = new THREE.Quaternion()
const refQ = new THREE.Quaternion()

/** The guard, in the chest's frame: blade upright a touch forward and out, flat toward the viewer. */
const KAMAE_AXIS = new THREE.Vector3(-0.16, 1, 0.1).normalize()
const KAMAE_EDGE = new THREE.Vector3(0.4, 0, 0.92).normalize()
/**
 * Hilt centre in arm lengths from between the shoulders: forward of the face, a little low and to
 * her right, where both hands reach it with the elbows still bent.
 */
const KAMAE_AT = new THREE.Vector3(-0.1, -0.25, 0.55)
/** Without a second arm to measure: in torso lengths (chest → head) from the chest. */
const KAMAE_AT_TORSO = new THREE.Vector3(-0.22, 0.36, 0.85)
const KAMAE_LOCAL = gripQuaternionInto(new THREE.Quaternion(), KAMAE_AXIS, KAMAE_EDGE)
/** Let go: the blade tips forward over the guard and falls point-down. */
const DROP_LOCAL = new THREE.Quaternion()
  .setFromAxisAngle(new THREE.Vector3(1, 0, 0), Math.PI - 0.15)
  .multiply(KAMAE_LOCAL)
const DROP_FALL = 0.12

function arm(vrm: VRM, side: Side): Arm | null {
  const node = (name: string) => vrm.humanoid?.getNormalizedBoneNode(name as VRMHumanBoneName) ?? undefined
  const hand = node(`${side}Hand`)
  if (!hand) return null
  const curls: Arm['curls'] = []
  for (const finger of FINGERS) {
    JOINTS.forEach((joint, i) => {
      const bone = node(`${side}${finger}${joint}`)
      if (bone) curls.push({ node: bone, flex: HILT_CURL[i] * FINGER_SHARE[finger] })
    })
  }
  return {
    side: side === 'left' ? 1 : -1,
    upper: node(`${side}UpperArm`),
    lower: node(`${side}LowerArm`),
    hand,
    index: node(`${side}IndexProximal`),
    middle: node(`${side}MiddleProximal`),
    little: node(`${side}LittleProximal`) ?? node(`${side}RingProximal`),
    curls,
    thumbBase: node(`${side}ThumbMetacarpal`),
    thumb: node(`${side}ThumbProximal`),
    thumbTip: node(`${side}ThumbDistal`),
  }
}

function knuckle(
  outVec: THREE.Vector3,
  a: Arm,
  bone: THREE.Object3D | undefined,
  key: keyof typeof FALLBACK_KNUCKLE,
): void {
  if (bone) {
    bone.getWorldPosition(outVec)
    return
  }
  const [along, across] = FALLBACK_KNUCKLE[key]
  a.hand.localToWorld(outVec.set(a.side * along, 0, across))
}

/** The palm frame of `a` from its current world pose. */
function palmFrame(a: Arm): GripFrame {
  ;(a.middle ?? a.hand).updateWorldMatrix(true, false)
  a.index?.updateWorldMatrix(false, false)
  a.little?.updateWorldMatrix(false, false)
  a.hand.getWorldPosition(wrist)
  knuckle(knuckleI, a, a.index, 'index')
  knuckle(knuckleM, a, a.middle, 'middle')
  knuckle(knuckleL, a, a.little, 'little')
  return gripFrameInto(frame, wrist, knuckleI, knuckleM, knuckleL, a.side)
}

/** Rotates `bone` in world space by `delta` scaled to `weight`, keeping its parent where it is. */
function turnWorld(bone: THREE.Object3D, delta: THREE.Quaternion, weight: number): void {
  qc.identity().slerp(delta, weight)
  bone.getWorldQuaternion(qd).premultiply(qc)
  bone.parent?.getWorldQuaternion(qc)
  bone.quaternion.copy(qc.invert().multiply(qd))
  bone.updateWorldMatrix(false, true)
}

function aim(bone: THREE.Object3D, fromDir: THREE.Vector3, toDir: THREE.Vector3, weight: number): void {
  qa.setFromUnitVectors(fromDir.normalize(), toDir.normalize())
  turnWorld(bone, qa, weight)
}

function curl(a: Arm, weight: number): void {
  if (weight <= 0) return
  const sign = -a.side
  for (const { node, flex } of a.curls) node.rotation.z += (sign * flex - node.rotation.z) * weight
  // The thumb points forward (+Z) in the rest pose: it flexes about X to close over the hilt, and
  // swings in (Y) toward the index so it lands on the fingers instead of along the hilt.
  if (a.thumb) {
    a.thumb.rotation.x += (THUMB_FLEX - a.thumb.rotation.x) * weight
    a.thumb.rotation.y += (sign * 0.35 - a.thumb.rotation.y) * weight
  }
  if (a.thumbTip) a.thumbTip.rotation.x += (THUMB_FLEX * 0.8 - a.thumbTip.rotation.x) * weight
  if (a.thumbBase) a.thumbBase.rotation.z += (-sign * 0.3 - a.thumbBase.rotation.z) * weight
}

export class HiltGrip {
  /** Where the katana root goes this frame (world); valid while `holding`. */
  readonly position = new THREE.Vector3()
  readonly quaternion = new THREE.Quaternion()
  private sword?: Arm
  private free?: Arm
  private ref?: THREE.Object3D
  private torso = 0.35

  attach(vrm: VRM): void {
    const right = arm(vrm, 'right')
    const left = arm(vrm, 'left')
    this.sword = right ?? left ?? undefined
    this.free = right && left ? left : undefined
    const humanoid = vrm.humanoid
    this.ref =
      humanoid?.getNormalizedBoneNode('chest') ??
      humanoid?.getNormalizedBoneNode('spine') ??
      this.sword?.hand.parent ??
      undefined
    const head = humanoid?.getNormalizedBoneNode('head')
    if (head && this.ref) {
      head.updateWorldMatrix(true, false)
      const rise = head.getWorldPosition(from).y - this.ref.getWorldPosition(to).y
      if (rise > 0.1) this.torso = rise
    }
  }

  get attached(): boolean {
    return Boolean(this.sword)
  }

  /** Writes the katana transform and bends the arms onto it; false when there is no hand to hold it. */
  solve(w: Readonly<HoldWeights>): boolean {
    const sword = this.sword
    if (!sword || w.hold <= 0) return false
    const held = 1 - w.release
    this.inHand(sword)
    this.blendToKamae(w.raise, w.release)
    this.place(sword, RIGHT_ON_HILT, held)
    if (this.free) this.place(this.free, LEFT_ON_HILT, w.join * held)
    curl(sword, Math.min(1, w.hold * 1.5) * held)
    if (this.free) curl(this.free, w.join * held)
    return true
  }

  /** The katana where the sword hand holds it now: its palm on the hilt, blade out past the thumb. */
  private inHand(sword: Arm): void {
    const f = palmFrame(sword)
    gripQuaternionInto(this.quaternion, f.axis, f.normal)
    this.position.copy(f.center).addScaledVector(axis.set(0, 1, 0).applyQuaternion(this.quaternion), -RIGHT_ON_HILT)
  }

  private blendToKamae(raise: number, release: number): void {
    const ref = this.ref
    if (!ref || (raise <= 0 && release <= 0)) return
    ref.updateWorldMatrix(true, false)
    ref.getWorldQuaternion(refQ)
    this.kamaeCenter(ref)
    kamaeQ.copy(refQ).multiply(KAMAE_LOCAL)
    this.position.lerp(kamaePos, raise)
    this.quaternion.slerp(kamaeQ, raise)
    if (release <= 0) return
    this.quaternion.slerp(qb.copy(refQ).multiply(DROP_LOCAL), release)
    this.position.addScaledVector(up.set(0, 1, 0).applyQuaternion(refQ), -DROP_FALL * this.torso * release)
  }

  /** Where the guard holds the hilt: measured from the shoulders and arm length when both arms exist. */
  private kamaeCenter(ref: THREE.Object3D): void {
    const sword = this.sword
    const free = this.free
    if (!sword?.upper || !free?.upper || !free.lower) {
      kamaePos.copy(KAMAE_AT_TORSO).multiplyScalar(this.torso).applyQuaternion(refQ).add(ref.getWorldPosition(to))
      return
    }
    sword.upper.getWorldPosition(from)
    free.upper.getWorldPosition(to)
    free.lower.getWorldPosition(rootPos)
    free.hand.getWorldPosition(midPos)
    const reach = to.distanceTo(rootPos) + rootPos.distanceTo(midPos)
    kamaePos.copy(KAMAE_AT).multiplyScalar(reach).applyQuaternion(refQ).add(from.add(to).multiplyScalar(0.5))
  }

  /** Two-bone IK to bring `a`'s palm onto the hilt at `along`, then turns the hand to wrap it. */
  private place(a: Arm, along: number, weight: number): void {
    if (weight <= 0.001 || !a.upper || !a.lower) return
    axis.set(0, 1, 0).applyQuaternion(this.quaternion)
    normal.set(1, 0, 0).applyQuaternion(this.quaternion)
    spot.copy(this.position).addScaledVector(axis, along)
    const f = palmFrame(a)
    // delta = where the palm must turn to (hilt frame) × where it faces now⁻¹.
    gripQuaternionInto(qb, axis, normal).multiply(gripQuaternionInto(qa, f.axis, f.normal).invert())
    a.hand.getWorldPosition(endPos)
    from.subVectors(f.center, endPos).applyQuaternion(qb)
    const target = spot.sub(from)
    a.upper.getWorldPosition(rootPos)
    a.lower.getWorldPosition(midPos)
    // Elbows hang down and a little out, the way a sword arm settles.
    this.ref?.getWorldQuaternion(refQ)
    out.set(a.side, 0, 0).applyQuaternion(refQ)
    up.set(0, 1, 0).applyQuaternion(refQ)
    pole.copy(midPos).addScaledVector(up, -0.25).addScaledVector(out, 0.15)
    solveTwoBoneInto(newMid, newEnd, rootPos, midPos, endPos, target, pole)
    aim(a.upper, from.subVectors(midPos, rootPos), to.subVectors(newMid, rootPos), weight)
    a.lower.getWorldPosition(midPos)
    a.hand.getWorldPosition(endPos)
    aim(a.lower, from.subVectors(endPos, midPos), to.subVectors(newEnd, newMid), weight)
    const turn = palmFrame(a)
    gripQuaternionInto(qb, axis, normal).multiply(gripQuaternionInto(qd, turn.axis, turn.normal).invert())
    turnWorld(a.hand, qb, weight)
  }
}
