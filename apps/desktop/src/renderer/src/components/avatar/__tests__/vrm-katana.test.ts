import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { gripFrameInto } from '../vrm-grip'
import { SCATTER_AT, VrmKatana } from '../vrm-katana'
import { REDUCED_PETALS } from '../vrm-senbonzakura'

function rig(handBone = 'rightHand') {
  const scene = new THREE.Scene()
  const hand = new THREE.Object3D()
  hand.position.set(0.2, 1.1, 0.25)
  scene.add(hand)
  const vrm = { humanoid: { getNormalizedBoneNode: (name: string) => (name === handBone ? hand : null) } }
  let scattered = 0
  const katana = new VrmKatana(scene, () => scattered++)
  katana.attach(vrm as unknown as VRM)
  return { katana, scene, scattered: () => scattered }
}

/** Plays senbonzakura from 0 up to `until`, one frame per 1/60 s of a 5 s action — hold before the
 * VRM update, meshes after, the adapter's order. */
function playUntil(katana: VrmKatana, until: number, action = 'senbonzakura') {
  for (let p = 0; p <= until; p += 1 / 300) {
    katana.hold(action, p)
    katana.update(1 / 60, action, p)
  }
}

function finish(katana: VrmKatana) {
  for (let i = 0; i < 240; i++) katana.update(1 / 60, undefined, 1)
}

describe('katana da senbonzakura', () => {
  it('a lâmina aparece na mão antes de se desfazer', () => {
    const { katana } = rig()
    playUntil(katana, 0.3)
    expect(katana.bladeVisible).toBe(true)
    expect(katana.visiblePetals).toBe(0)
  })

  it('aponta a lâmina para cima ao erguer e para baixo antes de soltar', () => {
    const { katana } = rig()
    playUntil(katana, 0.34)
    expect(katana.bladeDirection().y).toBeGreaterThan(0.5)
    playUntil(katana, SCATTER_AT - 0.01)
    expect(katana.bladeDirection().y).toBeLessThan(-0.5)
  })

  it('no instante da soltura a lâmina vira pétalas e o efeito sakura dispara uma vez', () => {
    const { katana, scattered } = rig()
    playUntil(katana, 0.7)
    expect(katana.bladeVisible).toBe(false)
    expect(katana.visiblePetals).toBeGreaterThanOrEqual(800)
    expect(scattered()).toBe(1)
  })

  it('termina limpo: nada fica na cena quando a ação acaba', () => {
    const { katana } = rig()
    playUntil(katana, 0.7)
    katana.update(1 / 60, undefined, 1)
    for (let i = 0; i < 120; i++) katana.update(1 / 60, undefined, 1)
    expect(katana.bladeVisible).toBe(false)
    expect(katana.visiblePetals).toBe(0)
  })

  it('outra ação não mostra espada nem pétalas', () => {
    const { katana, scattered } = rig()
    playUntil(katana, 0.9, 'wave')
    expect(katana.bladeVisible).toBe(false)
    expect(katana.visiblePetals).toBe(0)
    expect(scattered()).toBe(0)
  })
})

describe('ciclo de vida da katana', () => {
  it('a espada e as pétalas só existem na cena durante o golpe', () => {
    const { katana, scene } = rig()
    const before = scene.children.length
    playUntil(katana, 0.2)
    expect(scene.children.length).toBeGreaterThan(before)
    playUntil(katana, 0.99)
    finish(katana)
    expect(scene.children.length).toBe(before)
  })

  it('sem mão direita no rig, empunha com a esquerda', () => {
    const { katana } = rig('leftHand')
    playUntil(katana, 0.3)
    expect(katana.bladeVisible).toBe(true)
  })

  it('pétalas somam luz (blending aditivo), prontas para o bloom', () => {
    const { katana, scene } = rig()
    playUntil(katana, 0.6)
    const petals = scene.children.find((c) => (c as THREE.InstancedMesh).isInstancedMesh) as THREE.InstancedMesh
    expect((petals.material as THREE.Material).blending).toBe(THREE.AdditiveBlending)
    expect(petals.count).toBe(katana.visiblePetals)
  })

  it('movimento reduzido: poucas pétalas e sem a tempestade de tela cheia', () => {
    const { katana, scattered } = rig()
    katana.setReducedMotion(true)
    playUntil(katana, 0.6)
    expect(katana.visiblePetals).toBeGreaterThan(0)
    expect(katana.visiblePetals).toBeLessThanOrEqual(REDUCED_PETALS)
    expect(scattered()).toBe(0)
  })
})

type Bones = Map<string, THREE.Object3D>

function bone(bones: Bones, name: string, parent: THREE.Object3D, x: number, y: number, z: number): THREE.Object3D {
  const node = new THREE.Object3D()
  node.position.set(x, y, z)
  parent.add(node)
  bones.set(name, node)
  return node
}

const KNUCKLES = { Index: [0.08, 0.025], Middle: [0.085, 0], Ring: [0.08, -0.02], Little: [0.075, -0.035] } as const

/** A humanoid skeleton the size of an adult, arms hanging at rest, facing +Z like a loaded VRM. */
function skeleton(): { scene: THREE.Scene; bones: Bones; vrm: VRM } {
  const scene = new THREE.Scene()
  const bones: Bones = new Map()
  const hips = bone(bones, 'hips', scene, 0, 0.95, 0)
  const chest = bone(bones, 'chest', bone(bones, 'spine', hips, 0, 0.1, 0), 0, 0.2, 0)
  bone(bones, 'head', bone(bones, 'neck', chest, 0, 0.25, 0), 0, 0.1, 0)
  for (const [side, x] of [
    ['left', 1],
    ['right', -1],
  ] as const) {
    const upper = bone(
      bones,
      `${side}UpperArm`,
      bone(bones, `${side}Shoulder`, chest, x * 0.05, 0.18, 0),
      x * 0.12,
      0,
      0,
    )
    upper.rotation.z = x * -1.45
    const hand = bone(bones, `${side}Hand`, bone(bones, `${side}LowerArm`, upper, x * 0.24, 0, 0), x * 0.23, 0, 0)
    for (const [finger, [along, across]] of Object.entries(KNUCKLES)) {
      const proximal = bone(bones, `${side}${finger}Proximal`, hand, x * along, 0, across)
      bone(
        bones,
        `${side}${finger}Distal`,
        bone(bones, `${side}${finger}Intermediate`, proximal, x * 0.03, 0, 0),
        x * 0.02,
        0,
        0,
      )
    }
  }
  scene.updateMatrixWorld(true)
  const vrm = { humanoid: { getNormalizedBoneNode: (name: string) => bones.get(name) ?? null } }
  return { scene, bones, vrm: vrm as unknown as VRM }
}

const world = (bones: Bones, name: string) => (bones.get(name) as THREE.Object3D).getWorldPosition(new THREE.Vector3())

function palm(bones: Bones, side: 'left' | 'right'): THREE.Vector3 {
  const frame = { center: new THREE.Vector3(), axis: new THREE.Vector3(), normal: new THREE.Vector3() }
  const at = (joint: string) => world(bones, `${side}${joint}`)
  gripFrameInto(
    frame,
    at('Hand'),
    at('IndexProximal'),
    at('MiddleProximal'),
    at('LittleProximal'),
    side === 'left' ? 1 : -1,
  )
  return frame.center
}

/** Hilt centreline of the drawn katana, in world space. */
function hilt(scene: THREE.Scene): THREE.Line3 {
  const root = scene.children.find((c) => (c as THREE.Group).isGroup) as THREE.Group
  root.updateMatrixWorld(true)
  return new THREE.Line3(
    root.localToWorld(new THREE.Vector3(0, -0.12, 0)),
    root.localToWorld(new THREE.Vector3(0, 0.12, 0)),
  )
}

function offHilt(line: THREE.Line3, point: THREE.Vector3): number {
  return line.closestPointToPoint(point, true, new THREE.Vector3()).distanceTo(point)
}

describe('empunhadura da katana', () => {
  it('no saque o cabo está na palma direita, não flutuando ao lado da mão', () => {
    const { scene, bones, vrm } = skeleton()
    const katana = new VrmKatana(scene, () => {})
    katana.attach(vrm)
    playUntil(katana, 0.2)
    expect(offHilt(hilt(scene), palm(bones, 'right'))).toBeLessThan(0.01)
  })

  it('em guarda as duas mãos fecham no cabo, a direita junto à tsuba e a esquerda abaixo', () => {
    const { scene, bones, vrm } = skeleton()
    const katana = new VrmKatana(scene, () => {})
    katana.attach(vrm)
    playUntil(katana, 0.34)
    const line = hilt(scene)
    const right = palm(bones, 'right')
    const left = palm(bones, 'left')
    expect(offHilt(line, right)).toBeLessThan(0.01)
    expect(offHilt(line, left)).toBeLessThan(0.01)
    expect(line.closestPointToPointParameter(right, true)).toBeGreaterThan(
      line.closestPointToPointParameter(left, true),
    )
  })

  it('erguida diante do rosto, a lâmina não atravessa a cabeça', () => {
    const { scene, bones, vrm } = skeleton()
    const katana = new VrmKatana(scene, () => {})
    katana.attach(vrm)
    playUntil(katana, 0.34)
    const root = scene.children.find((c) => (c as THREE.Group).isGroup) as THREE.Group
    const blade = new THREE.Line3(
      root.localToWorld(new THREE.Vector3(0, 0.13, 0)),
      root.localToWorld(new THREE.Vector3(0, 0.9, 0)),
    )
    const head = world(bones, 'head')
    expect(offHilt(blade, head)).toBeGreaterThan(0.15)
  })
})
