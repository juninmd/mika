import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { VrmActionClock } from '../vrm-action-clock'
import { VrmBodyDriver } from '../vrm-body-driver'
import { HEAD_BACK_LIMIT } from '../vrm-gaze'

function fakeVrm() {
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
  return { vrm: { scene, humanoid } as unknown as VRM, head: () => bones.get('head') as THREE.Object3D }
}

/** Lowest head pitch (most tilted back) over `seconds` of `action`, with the cursor above her. */
function mostTiltedBack(action: string, seconds: number): number {
  const { vrm, head } = fakeVrm()
  const driver = new VrmBodyDriver(new THREE.Scene())
  const clock = new VrmActionClock()
  driver.setCursorTarget(0, -1)
  driver.play(action)
  let lowest = 0
  const dt = 1 / 60
  for (let i = 0; i < seconds * 60; i++) {
    driver.update(vrm, dt, i * dt, clock, false, 0, false)
    lowest = Math.min(lowest, head().rotation.x)
  }
  return lowest
}

// Smoke 2026-09-27: sampled live, the head was >5° back 16% of the idle time and reached 26.5°
// (drink sips + yawn/stretch poses + gaze up, all additive on the same bone, none capped).
describe('a cabeça do VRM não fica jogada para trás', () => {
  for (const action of ['drink', 'yawn', 'stretch', 'cheer']) {
    it(`${action} com o cursor acima dela não passa do limite`, () => {
      expect(mostTiltedBack(action, 4)).toBeGreaterThanOrEqual(-HEAD_BACK_LIMIT - 1e-6)
    })
  }
})
