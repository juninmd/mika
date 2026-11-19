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

// Gesto procedural parava o idle capturado inteiro: as pernas iam para a pose de manequim (juntas e
// retas) a cada aceno. Agora o clipe segura quadril e pernas enquanto o gesto é dono do tronco.
describe('gesto procedural sobre a postura capturada', () => {
  function twoBones() {
    const root = new THREE.Object3D()
    const arm = new THREE.Object3D()
    const leg = new THREE.Object3D()
    arm.name = 'arm'
    leg.name = 'leg'
    root.add(arm, leg)
    // A slow drift, not a constant: three's mixer skips rewriting a value that did not change.
    const bend = (angle: number) => new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), angle).toArray()
    const hold = (bone: string) =>
      new THREE.QuaternionKeyframeTrack(`${bone}.quaternion`, [0, 10], [...bend(0.4), ...bend(0.5)])
    const layer = new VrmClipLayer()
    const clip = new THREE.AnimationClip('idle', 10, [hold('arm'), hold('leg')])
    layer.adopt(root, undefined, new Map([['idle', clip]]), new Set([leg]))
    const angle = (bone: THREE.Object3D) => 2 * Math.acos(Math.min(1, Math.abs(bone.quaternion.w)))
    const step = () => {
      arm.quaternion.identity()
      leg.quaternion.identity()
      layer.blend(FRAME)
      return { arm: angle(arm), leg: angle(leg) }
    }
    return { layer, step }
  }

  it('o clipe fica nas pernas e devolve o braço ao gesto, sem salto', () => {
    const { layer, step } = twoBones()
    layer.play('idle', { loop: true, fade: 0.3 })
    let prev = step()
    for (let t = 0; t < 1; t += FRAME) prev = step()
    layer.setUpperBody(false, 0.3)
    let worst = 0
    for (let t = 0; t < 1; t += FRAME) {
      const now = step()
      worst = Math.max(worst, Math.abs(now.arm - prev.arm))
      prev = now
    }
    expect(prev.leg).toBeGreaterThan(0.4)
    expect(prev.arm).toBeLessThan(1e-3)
    expect(worst).toBeLessThan(0.05)
  })
})

// O mixer do three só reescreve um osso quando o valor do clipe muda. Num trecho parado da captura, o
// rig procedural reescrevia o repouso e o clipe não voltava: o corpo "teletransportava" para o repouso.
describe('trecho parado do clipe', () => {
  it('osso e quadril seguem o clipe mesmo quando a captura fica parada', () => {
    const root = new THREE.Object3D()
    const arm = new THREE.Object3D()
    const hips = new THREE.Object3D()
    arm.name = 'arm'
    hips.name = 'hips'
    root.add(arm, hips)
    const bent = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), 0.6).toArray()
    const turn = new THREE.QuaternionKeyframeTrack('arm.quaternion', [0, 1, 4], [0, 0, 0, 1, ...bent, ...bent])
    const shift = new THREE.VectorKeyframeTrack('hips.position', [0, 1, 4], [0, 0, 0, 0.2, 0, 0, 0.2, 0, 0])
    const layer = new VrmClipLayer()
    layer.adopt(root, hips, new Map([['hold', new THREE.AnimationClip('hold', 4, [turn, shift])]]))
    layer.play('hold', { fade: 0.2 })
    for (let t = 0; t < 2; t += FRAME) {
      arm.quaternion.identity()
      layer.resetRoot()
      layer.blend(FRAME)
    }
    expect(2 * Math.acos(Math.min(1, Math.abs(arm.quaternion.w)))).toBeCloseTo(0.6, 2)
    expect(hips.position.x).toBeCloseTo(0.2, 3)
  })
})

function held() {
  const root = new THREE.Object3D()
  const bone = new THREE.Object3D()
  bone.name = 'arm'
  root.add(bone)
  const clip = (name: string, angle: number) => {
    const q = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), angle).toArray()
    return new THREE.AnimationClip(name, 10, [
      new THREE.QuaternionKeyframeTrack('arm.quaternion', [0, 10], [...q, ...q]),
    ])
  }
  const layer = new VrmClipLayer()
  layer.adopt(
    root,
    undefined,
    new Map([
      ['a', clip('a', 0)],
      ['b', clip('b', 1.2)],
      ['c', clip('c', 0.3)],
    ]),
  )
  const step = () => {
    bone.quaternion.identity()
    layer.blend(FRAME)
    return 2 * Math.acos(Math.min(1, Math.abs(bone.quaternion.w)))
  }
  return { layer, step }
}

// O three agenda o fade-out de `crossFadeFrom` a partir do peso 1: um gesto que chegava no meio do
// crossfade do idle levava o idle de 0,1 para 1 num quadro só — o "teletransporte" entre gestos.
describe('gesto que interrompe um crossfade', () => {
  it('parte da pose visível, sem salto', () => {
    const { layer, step } = held()
    layer.play('a', { loop: true, fade: 0.05 })
    for (let t = 0; t < 1; t += FRAME) step()
    layer.play('b', { loop: true, fade: 0.8 })
    let prev = 0
    for (let i = 0; i < 6; i++) prev = step()
    layer.play('c', { fade: 0.35 })
    let worst = 0
    for (let i = 0; i < 40; i++) {
      const angle = step()
      worst = Math.max(worst, Math.abs(angle - prev))
      prev = angle
    }
    expect(worst).toBeLessThan(0.05)
  })

  // A rampa linear do three largava o braço a toda velocidade e o parava seco ao fim dos 0,35 s.
  it('o crossfade acelera e freia', () => {
    const { layer, step } = held()
    layer.play('a', { loop: true, fade: 0.05 })
    let prev = 0
    for (let t = 0; t < 1; t += FRAME) prev = step()
    layer.play('b', { loop: true, fade: 0.35 })
    const speeds: number[] = []
    for (let i = 0; i < 30; i++) {
      const angle = step()
      speeds.push(Math.abs(angle - prev))
      prev = angle
    }
    const peak = Math.max(...speeds)
    expect(speeds[0]).toBeLessThan(peak * 0.25)
    expect(speeds[20]).toBeLessThan(peak * 0.25)
  })
})

// Achados da revisão do PR #58: casos de borda do crossfade suave.
describe('crossfade suave nas bordas', () => {
  // `twin` alterna entre duas ações; no terceiro replay a "nova" ação ainda estava saindo e era zerada.
  it('repetir o mesmo clipe três vezes seguidas não salta', () => {
    const { layer, step } = held()
    layer.play('b', { fade: 0.05 })
    for (let t = 0; t < 1; t += FRAME) step()
    let worst = 0
    for (let play = 0; play < 3; play++) {
      layer.play('b', { fade: 0.35 })
      for (let i = 0; i < 6; i++) worst = Math.max(worst, Math.abs(step() - 1.2))
    }
    expect(worst).toBeLessThan(0.05)
  })

  // span 0 virava 0/0 = NaN no peso: o clipe nunca entrava.
  it('fade zero entra na hora', () => {
    const { layer, step } = held()
    layer.play('a', { loop: true, fade: 0.05 })
    for (let t = 0; t < 1; t += FRAME) step()
    layer.play('b', { loop: true, fade: 0 })
    let angle = 0
    for (let t = 0; t < 0.5; t += FRAME) angle = step()
    expect(angle).toBeCloseTo(1.2, 2)
  })
})
