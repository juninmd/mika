import * as THREE from 'three'

export type ShowcaseShot = 'stage' | 'bust'

/** Share of the model's height, from the top of the head down, that a bust shot keeps. */
const BUST_SPAN = 0.36
/** Breathing room around the framed span, so hair and shoes never touch the edge. */
const MARGIN: Record<ShowcaseShot, number> = { stage: 1.05, bust: 1.06 }
/** Camera sits a touch above the framed centre and looks slightly down, like a studio portrait. */
const ELEVATION = 0.06

const target = new THREE.Vector3()

// Character-select framing: the camera orbits the model at `yaw` (0 = front, +Z) and fits either
// the whole body (`stage`) or head-to-chest (`bust`) in the frustum's vertical span. `pivot` (the
// head) sets the horizontal centre, so wind-blown hair widening the bounds cannot push the face aside.
export function frameShowcase(
  camera: THREE.PerspectiveCamera,
  bounds: THREE.Box3,
  shot: ShowcaseShot,
  yaw: number,
  pivot?: THREE.Vector3,
): void {
  const height = bounds.max.y - bounds.min.y
  const span = shot === 'bust' ? height * BUST_SPAN : height
  const width = bounds.max.x - bounds.min.x
  const depth = bounds.max.z - bounds.min.z
  const top = bounds.max.y
  target.set(
    pivot?.x ?? (bounds.min.x + bounds.max.x) / 2,
    top - span / 2,
    pivot?.z ?? (bounds.min.z + bounds.max.z) / 2,
  )
  const tanV = Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2)
  const fitH = span / (2 * tanV)
  const fitW = shot === 'bust' ? 0 : width / (2 * tanV * Math.max(0.1, camera.aspect))
  const distance = Math.max(fitH, fitW) * MARGIN[shot] + depth / 2
  camera.position.set(
    target.x + Math.sin(yaw) * distance,
    target.y + span * ELEVATION,
    target.z + Math.cos(yaw) * distance,
  )
  camera.lookAt(target)
  camera.near = Math.max(0.05, distance - depth - 1)
  camera.far = distance + depth + 5
  camera.updateProjectionMatrix()
}
