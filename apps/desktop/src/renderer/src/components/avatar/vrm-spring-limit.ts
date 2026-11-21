import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import type { GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js'

/**
 * VRMC_springBone_limit: the range a joint's tail may swing in (cone, hinge or spherical). three-vrm
 * 3.5 reads VRMC_springBone but ignores this extension, so a model that relies on it lets its hair
 * swing into the body. Ported from the spec's reference math as VRMKit implements it.
 */
type Limit =
  | { kind: 'cone'; angle: number; rotation: THREE.Quaternion }
  | { kind: 'hinge'; angle: number; rotation: THREE.Quaternion }
  | { kind: 'spherical'; pitch: number; yaw: number; rotation: THREE.Quaternion }

type LimitJson = Partial<Record<'cone' | 'hinge' | 'spherical', Record<string, unknown>>>

/** three-vrm internals the limit hooks into; pinned to @pixiv/three-vrm 3.5.5 and proven by mika-hair-collision.test.ts. */
type JointInternals = {
  bone: THREE.Object3D
  _collision(tail: THREE.Vector3): void
  _boneAxis: THREE.Vector3
  _initialLocalMatrix: THREE.Matrix4
  _parentMatrixWorld: THREE.Matrix4
  _worldSpaceBoneLength: number
}

const EPS = 1e-6
const angle = (value: unknown, max: number): number =>
  Math.min(Math.max(Number.isFinite(value) ? Number(value) : 0, 0), max)

function parseLimit(json: LimitJson | undefined): Limit | undefined {
  const shape = json?.cone ?? json?.hinge ?? json?.spherical
  if (!json || !shape) return undefined
  const r = Array.isArray(shape.rotation) ? shape.rotation.map(Number) : [0, 0, 0, 1]
  const rotation = new THREE.Quaternion(r[0], r[1], r[2], r[3])
  if (!(rotation.length() > EPS)) return undefined
  rotation.normalize()
  if (json.cone) return { kind: 'cone', angle: angle(shape.angle, Math.PI), rotation }
  if (json.hinge) return { kind: 'hinge', angle: angle(shape.angle, Math.PI), rotation }
  return { kind: 'spherical', pitch: angle(shape.pitch, Math.PI), yaw: angle(shape.yaw, Math.PI / 2), rotation }
}

function cone(d: THREE.Vector3, max: number): void {
  const cos = Math.cos(max)
  if (d.y >= cos) return
  const sin = Math.sqrt(1 - cos * cos)
  const horizontal = Math.hypot(d.x, d.z)
  if (horizontal <= EPS) d.set(0, cos, sin)
  else d.set((d.x * sin) / horizontal, cos, (d.z * sin) / horizontal)
}

function hinge(d: THREE.Vector3, max: number): void {
  const length = Math.hypot(d.y, d.z)
  if (length <= EPS) return void d.set(0, 1, 0)
  const [y, z] = [d.y / length, d.z / length]
  const cos = Math.cos(max)
  if (y >= cos) return void d.set(0, y, z)
  d.set(0, cos, Math.sign(z || 1) * Math.sqrt(1 - cos * cos))
}

function spherical(d: THREE.Vector3, pitchMax: number, yawMax: number): void {
  const pitch = d.y <= -1 + EPS ? Math.PI : Math.abs(d.x) >= 1 - EPS ? 0 : Math.atan2(d.z, d.y)
  const yaw = Math.asin(THREE.MathUtils.clamp(d.x, -1, 1))
  const p = THREE.MathUtils.clamp(pitch, -pitchMax, pitchMax)
  const y = THREE.MathUtils.clamp(yaw, -yawMax, yawMax)
  d.set(Math.sin(y), Math.cos(y) * Math.cos(p), Math.cos(y) * Math.sin(p))
}

const _space = new THREE.Quaternion()
const _axisTurn = new THREE.Quaternion()
const _rest = new THREE.Quaternion()
const _head = new THREE.Vector3()
const _dir = new THREE.Vector3()
const _m = new THREE.Matrix4()
const _s = new THREE.Vector3()

/** World rotation of the limit's space: rest rotation, then +y turned onto the bone axis, then the limit's own rotation. */
function limitSpace(joint: JointInternals, limit: Limit): THREE.Quaternion {
  _m.multiplyMatrices(joint._parentMatrixWorld, joint._initialLocalMatrix).decompose(_head, _rest, _s)
  const a = joint._boneAxis
  if (a.y <= -1 + EPS) _axisTurn.set(1, 0, 0, 0)
  else _axisTurn.set(a.z, 0, -a.x, a.y + 1).normalize()
  return _space.copy(_rest).multiply(_axisTurn).multiply(limit.rotation)
}

/** Moves `tail` (world space, bone length away from the head) to the nearest direction the limit allows. */
function constrain(joint: JointInternals, limit: Limit, tail: THREE.Vector3): void {
  const space = limitSpace(joint, limit)
  _head.setFromMatrixPosition(joint.bone.matrixWorld)
  _dir.subVectors(tail, _head).normalize().applyQuaternion(_rest.copy(space).invert())
  if (limit.kind === 'cone') cone(_dir, limit.angle)
  else if (limit.kind === 'hinge') hinge(_dir, limit.angle)
  else spherical(_dir, limit.pitch, limit.yaw)
  tail.copy(_dir.applyQuaternion(space)).multiplyScalar(joint._worldSpaceBoneLength).add(_head)
}

/** Reads each joint's VRMC_springBone_limit and enforces it after inertia and after colliders. Returns joints limited. */
export function applySpringBoneLimits(gltf: GLTF, vrm: VRM): number {
  const byNode = new Map<number, Limit>()
  for (const spring of gltf.parser.json.extensions?.VRMC_springBone?.springs ?? [])
    for (const joint of spring.joints ?? []) {
      const limit = parseLimit(joint.extensions?.VRMC_springBone_limit?.limit)
      if (limit) byNode.set(joint.node, limit)
    }
  let count = 0
  for (const joint of vrm.springBoneManager?.joints ?? []) {
    const node = gltf.parser.associations.get(joint.bone)?.nodes
    const limit = node === undefined ? undefined : byNode.get(node)
    if (!limit) continue
    // _collision is private in three-vrm's types; the cast reaches it on purpose.
    const internals = joint as unknown as JointInternals
    const collide = internals._collision.bind(joint)
    internals._collision = (tail: THREE.Vector3) => {
      constrain(internals, limit, tail)
      collide(tail)
      constrain(internals, limit, tail)
    }
    count++
  }
  return count
}
