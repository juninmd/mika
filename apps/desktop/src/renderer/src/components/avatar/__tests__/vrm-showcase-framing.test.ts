import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { frameShowcase } from '../vrm-showcase-framing'

// Stand-in silhouette: a 1.6 m standing model, feet on the floor, facing +Z like a loaded VRM.
const BOUNDS = new THREE.Box3(new THREE.Vector3(-0.35, 0, -0.15), new THREE.Vector3(0.35, 1.6, 0.15))

function project(camera: THREE.PerspectiveCamera, y: number): THREE.Vector3 {
  camera.updateMatrixWorld(true)
  return new THREE.Vector3(0, y, 0).project(camera)
}

function shoot(shot: 'stage' | 'bust', yaw = 0, aspect = 0.8): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(26, aspect, 0.1, 20)
  frameShowcase(camera, BOUNDS, shot, yaw)
  return camera
}

describe('frameShowcase', () => {
  it('stage shot keeps head and feet inside the frame and fills it', () => {
    const camera = shoot('stage')
    const head = project(camera, BOUNDS.max.y)
    const feet = project(camera, BOUNDS.min.y)
    expect(head.y).toBeLessThan(1)
    expect(feet.y).toBeGreaterThan(-1)
    expect(head.y - feet.y).toBeGreaterThan(1.4)
  })

  // The roster card is a portrait: the face must be big, so the legs are cut on purpose.
  it('bust shot keeps the top of the head in frame and crops below the chest', () => {
    const camera = shoot('bust')
    const head = project(camera, BOUNDS.max.y)
    expect(head.y).toBeLessThan(1)
    expect(head.y).toBeGreaterThan(0.6)
    expect(project(camera, BOUNDS.max.y * 0.55).y).toBeLessThan(-1)
    expect(project(camera, BOUNDS.min.y).y).toBeLessThan(-1)
  })

  it('orbits at a fixed distance, with yaw 0 in front of the model', () => {
    const front = shoot('stage', 0)
    const threeQuarter = shoot('stage', Math.PI / 5)
    const target = new THREE.Vector3(0, front.position.y, 0)
    expect(front.position.x).toBeCloseTo(0)
    expect(front.position.z).toBeGreaterThan(0)
    expect(threeQuarter.position.x).toBeGreaterThan(0)
    const flat = (v: THREE.Vector3) => Math.hypot(v.x, v.z)
    expect(flat(threeQuarter.position.clone().sub(target))).toBeCloseTo(flat(front.position.clone().sub(target)))
  })

  it('a wide stage never crops the model narrower than a tall one', () => {
    const wide = shoot('stage', 0, 1.6)
    const feet = project(wide, BOUNDS.min.y)
    expect(feet.y).toBeGreaterThan(-1)
  })

  // Wind-blown hair or an off-centre prop widens the bounds to one side; the face stays centred.
  it('centres the frame horizontally on the pivot when one is given', () => {
    const camera = new THREE.PerspectiveCamera(26, 0.8, 0.1, 20)
    const head = new THREE.Vector3(-0.12, 1.45, 0.02)
    frameShowcase(camera, BOUNDS, 'bust', 0, head)
    camera.updateMatrixWorld(true)
    expect(head.clone().project(camera).x).toBeCloseTo(0, 2)
  })
})
