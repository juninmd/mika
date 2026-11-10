import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { fitCamera } from '../vrm-scene-setup'

// A stand-in model: one box spanning the whole silhouette (hair/hat top to feet, arm span).
function fakeVrm(width: number, height: number): VRM {
  const scene = new THREE.Group()
  const body = new THREE.Mesh(new THREE.BoxGeometry(width, height, 0.3))
  body.position.y = height / 2
  scene.add(body)
  return { scene } as unknown as VRM
}

function projectedCorners(camera: THREE.PerspectiveCamera, vrm: VRM): THREE.Vector3[] {
  const box = new THREE.Box3().setFromObject(vrm.scene)
  const corners: THREE.Vector3[] = []
  for (const x of [box.min.x, box.max.x])
    for (const y of [box.min.y, box.max.y])
      for (const z of [box.min.z, box.max.z]) corners.push(new THREE.Vector3(x, y, z).project(camera))
  return corners
}

describe('fitCamera', () => {
  // The companion window is a fixed 400x640 box: any part of the model outside the frustum is
  // simply cut by the window edge (feet, hands of an A-pose, a tall hat).
  it.each([
    ['portrait window, standing model', 400, 640, 0.6, 1.6],
    ['portrait window, T-pose arm span', 400, 640, 1.6, 1.6],
    ['landscape window, standing model', 1280, 720, 0.6, 1.9],
  ])('keeps the whole model inside the window (%s)', (_label, w, h, modelW, modelH) => {
    const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 20)
    const vrm = fakeVrm(modelW, modelH)
    fitCamera(camera, w, h, vrm)
    camera.updateMatrixWorld(true)

    const corners = projectedCorners(camera, vrm)
    for (const c of corners) {
      expect(Math.abs(c.x)).toBeLessThanOrEqual(1)
      expect(Math.abs(c.y)).toBeLessThanOrEqual(1)
      expect(c.z).toBeLessThan(1)
    }
    // Fits, but still fills the window along its tight axis — not a speck in the middle.
    const spanX = Math.max(...corners.map((c) => c.x)) - Math.min(...corners.map((c) => c.x))
    const spanY = Math.max(...corners.map((c) => c.y)) - Math.min(...corners.map((c) => c.y))
    expect(Math.max(spanX, spanY)).toBeGreaterThan(1.5)
  })
})
