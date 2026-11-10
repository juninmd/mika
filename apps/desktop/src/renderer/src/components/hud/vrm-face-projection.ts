import * as THREE from 'three'
import type { AvatarFace } from '../../lib/avatar-face-bus'

/** Face box around the head bone, in meters (VRM units): the bone sits at the skull base. */
const FACE_CENTER_UP = 0.06
const FACE_HALF_WIDTH = 0.085
const FACE_HALF_HEIGHT = 0.12

const tmp = new THREE.Vector3()

function toScreen(point: THREE.Vector3, camera: THREE.Camera, w: number, h: number): { x: number; y: number } {
  tmp.copy(point).project(camera)
  return { x: ((tmp.x + 1) / 2) * w, y: ((1 - tmp.y) / 2) * h }
}

/** Projects her head bone to a CSS-px face rect for the HUD layout; null when off-screen or not loaded. */
export function projectVrmFace(
  frame: { head: THREE.Vector3 } | null,
  camera: THREE.Camera,
  width: number,
  height: number,
): AvatarFace | null {
  if (!frame || width <= 0 || height <= 0) return null
  const center = frame.head.clone().setY(frame.head.y + FACE_CENTER_UP)
  const c = toScreen(center, camera, width, height)
  const side = toScreen(center.clone().setX(center.x + FACE_HALF_WIDTH), camera, width, height)
  const top = toScreen(center.clone().setY(center.y + FACE_HALF_HEIGHT), camera, width, height)
  const hw = Math.abs(side.x - c.x)
  const hh = Math.abs(c.y - top.y)
  if (!Number.isFinite(hw + hh + c.x + c.y) || c.x < 0 || c.x > width || c.y < 0 || c.y > height) return null
  return { x: c.x - hw, y: c.y - hh, width: hw * 2, height: hh * 2 }
}
