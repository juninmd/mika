/**
 * Pure grip math: where a hilt sits in a hand, read from that rig's own bones (never a hardcoded
 * rotation — each model's hands are rolled differently), plus an analytic two-bone IK so the
 * second hand can reach the hilt. Plain vector math on caller-owned objects: nothing is allocated
 * per call.
 */
import * as THREE from 'three'

export interface GripFrame {
  /** Where the hilt's centreline crosses the palm. */
  center: THREE.Vector3
  /** Hilt direction: across the palm from the little finger to the index, toward the blade. */
  axis: THREE.Vector3
  /** Out of the palm, toward the hilt (the side the curled fingertips reach). */
  normal: THREE.Vector3
}

/** A real grip runs the hilt diagonally, from the heel of the hand to the index knuckle. */
const AXIS_TILT = 0.4
/** Share of wrist → middle knuckle where the hilt crosses the palm. */
const PALM_ALONG = 0.72
/** Hilt centre below the palm surface, as a share of the palm length (half a hand plus the hilt). */
const PALM_DEPTH = 0.34

const fingers = new THREE.Vector3()
const across = new THREE.Vector3()

/**
 * `side` is +1 for the left hand and -1 for the right: the palm normal is the cross of the finger
 * and thumb directions, mirrored between hands.
 */
export function gripFrameInto(
  out: GripFrame,
  wrist: THREE.Vector3,
  indexKnuckle: THREE.Vector3,
  middleKnuckle: THREE.Vector3,
  littleKnuckle: THREE.Vector3,
  side: 1 | -1,
): GripFrame {
  fingers.subVectors(middleKnuckle, wrist)
  const palm = fingers.length() || 1
  fingers.divideScalar(palm)
  across.subVectors(indexKnuckle, littleKnuckle)
  across.addScaledVector(fingers, -across.dot(fingers)).normalize()
  out.normal.crossVectors(fingers, across).multiplyScalar(side).normalize()
  out.axis.copy(across).addScaledVector(fingers, AXIS_TILT).normalize()
  out.center
    .copy(wrist)
    .addScaledVector(fingers, palm * PALM_ALONG)
    .addScaledVector(out.normal, palm * PALM_DEPTH)
  return out
}

const basis = new THREE.Matrix4()
const edge = new THREE.Vector3()
const side = new THREE.Vector3()

/** Rotation taking a sword's local +Y (blade) to `axis` and +X (edge) to `normal`, orthonormalized. */
export function gripQuaternionInto(
  out: THREE.Quaternion,
  axis: THREE.Vector3,
  normal: THREE.Vector3,
): THREE.Quaternion {
  edge.copy(normal).addScaledVector(axis, -normal.dot(axis)).normalize()
  side.crossVectors(edge, axis)
  basis.makeBasis(edge, axis, side)
  return out.setFromRotationMatrix(basis)
}

const reach = new THREE.Vector3()
const bend = new THREE.Vector3()
const EPS = 1e-4

/** Any unit vector perpendicular to `dir` (unit), into `out`. */
function perpendicular(out: THREE.Vector3, dir: THREE.Vector3): THREE.Vector3 {
  out.set(Math.abs(dir.y) < 0.9 ? 0 : 1, Math.abs(dir.y) < 0.9 ? 1 : 0, 0)
  return out.addScaledVector(dir, -out.dot(dir)).normalize()
}

/**
 * Law-of-cosines IK: writes the new elbow (`outMid`) and wrist (`outEnd`) so the chain root → mid
 * → end reaches `target` with its bone lengths kept, bending toward `pole`. Out of reach, the arm
 * straightens toward the target instead of stretching.
 */
export function solveTwoBoneInto(
  outMid: THREE.Vector3,
  outEnd: THREE.Vector3,
  root: THREE.Vector3,
  mid: THREE.Vector3,
  end: THREE.Vector3,
  target: THREE.Vector3,
  pole: THREE.Vector3,
): void {
  const a = root.distanceTo(mid)
  const b = mid.distanceTo(end)
  reach.subVectors(target, root)
  const d = Math.min(a + b - EPS, Math.max(Math.abs(a - b) + EPS, reach.length()))
  if (reach.lengthSq() < EPS * EPS) reach.subVectors(end, root)
  reach.normalize()
  bend.subVectors(pole, root).addScaledVector(reach, -bend.dot(reach))
  if (bend.lengthSq() < EPS * EPS) bend.subVectors(mid, root).addScaledVector(reach, -bend.dot(reach))
  if (bend.lengthSq() < EPS * EPS) perpendicular(bend, reach)
  bend.normalize()
  const cos = Math.min(1, Math.max(-1, (a * a + d * d - b * b) / (2 * a * d)))
  outMid
    .copy(root)
    .addScaledVector(reach, a * cos)
    .addScaledVector(bend, a * Math.sqrt(1 - cos * cos))
  outEnd.copy(root).addScaledVector(reach, d)
}
