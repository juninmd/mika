import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { VrmClipLayer } from '../vrm-clip-layer'

const FRAME = 1 / 60

/** One bone turning steadily on Z over a 10 s capture: easy to see where in the clip it is. */
function rig() {
  const root = new THREE.Object3D()
  const bone = new THREE.Object3D()
  bone.name = 'arm'
  root.add(bone)
  const end = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), 1.5)
  const track = new THREE.QuaternionKeyframeTrack('arm.quaternion', [0, 10], [0, 0, 0, 1, ...end.toArray()])
  const layer = new VrmClipLayer()
  layer.adopt(root, undefined, new Map([['long', new THREE.AnimationClip('long', 10, [track])]]))
  /** The procedural rig rewrites rest every frame before the clip blends over it. */
  const step = () => {
    bone.quaternion.identity()
    layer.blend(FRAME)
    return 2 * Math.acos(Math.min(1, Math.abs(bone.quaternion.w)))
  }
  return { layer, step }
}

describe('camada de clipes do VRM', () => {
  // As capturas duram 3,9 a 11,7 s e as ações 2,8 a 4,6 s: o corpo segurava o gesto muito depois do rosto.
  it('um clipe mais longo que a ação devolve o corpo quando a ação acaba', () => {
    const { layer, step } = rig()
    layer.play('long', { fade: 0.35, hold: 3 })
    for (let t = 0; t < 3.2; t += FRAME) step()
    expect(layer.playing).toBeUndefined()
  })

  // Repetir o mesmo gesto rebobinava a ação no lugar: o braço saltava para o primeiro quadro.
  it('repetir o mesmo clipe no meio dele não faz o osso saltar', () => {
    const { layer, step } = rig()
    layer.play('long', { fade: 0.35 })
    let prev = 0
    for (let t = 0; t < 5; t += FRAME) prev = step()
    layer.play('long', { fade: 0.35 })
    let worst = 0
    for (let i = 0; i < 40; i++) {
      const angle = step()
      worst = Math.max(worst, Math.abs(angle - prev))
      prev = angle
    }
    expect(worst).toBeLessThan(0.08)
  })
})
