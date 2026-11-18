import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { VrmActionClock } from '../vrm-action-clock'
import { VrmBodyDriver } from '../vrm-body-driver'

function fakeVrm(): { vrm: VRM; bones: Map<string, THREE.Object3D> } {
  const bones = new Map<string, THREE.Object3D>()
  const scene = new THREE.Object3D()
  const humanoid = {
    getNormalizedBoneNode(name: string) {
      if (!bones.has(name)) {
        const bone = new THREE.Object3D()
        bone.position.y = name === 'head' ? 1.4 : 1
        scene.add(bone)
        bones.set(name, bone)
      }
      return bones.get(name)
    },
  }
  return { vrm: { scene, humanoid } as unknown as VRM, bones }
}

function run(frames: number, end: number) {
  const { vrm, bones } = fakeVrm()
  const driver = new VrmBodyDriver(new THREE.Scene())
  const clock = new VrmActionClock()
  const dt = 1 / 60
  for (let i = frames - 1; i >= 0; i--) driver.update(vrm, dt, end - i * dt, clock, false, 0, false)
  return bones
}

// Additive motion over a bone nobody rewrites each frame accumulates: the hips spun her into profile.
describe('corpo do VRM não deriva com o tempo', () => {
  it('depois de um minuto cada osso está onde estaria no primeiro quadro do mesmo instante', () => {
    const long = run(3600, 60)
    const fresh = run(1, 60)
    for (const [name, bone] of long) {
      const reference = fresh.get(name)
      if (!reference) continue
      for (const axis of ['x', 'y', 'z'] as const) {
        expect(Math.abs(bone.rotation[axis] - reference.rotation[axis]), `${name}.${axis}`).toBeLessThan(0.03)
      }
    }
  })
})

// O balanço de respiração dos braços sumia no quadro em que um gesto começava: o braço estalava.
describe('gesto começa sem estalo no braço', () => {
  it('o antebraço não salta no quadro em que um gesto procedural assume', () => {
    const { vrm, bones } = fakeVrm()
    const driver = new VrmBodyDriver(new THREE.Scene())
    const clock = new VrmActionClock()
    const dt = 1 / 60
    let t = 0
    // ~1,5 s: a respiração está no pico, onde o balanço do antebraço é maior.
    for (; t < 1.5; t += dt) driver.update(vrm, dt, t, clock, false, 0, false)
    const before = bones.get('leftLowerArm')?.rotation.y ?? 0
    driver.play('wave')
    driver.update(vrm, dt, t, clock, false, 0, false)
    const after = bones.get('leftLowerArm')?.rotation.y ?? 0
    expect(Math.abs(after - before)).toBeLessThan(0.005)
  })
})
