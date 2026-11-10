import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'

// Procedural authoring: what tinting can't reach (hat, beret, scarf, ribbons, hood) becomes a three.js
// mesh pinned to the right humanoid bone. Each skin declares its pieces; the adapter mounts/unmounts them.
export type AccessoryKind = 'witch-hat' | 'beret' | 'scarf' | 'ribbons' | 'hood' | 'collar'

export interface AccessorySpec {
  kind: AccessoryKind
  color: string
  accent?: string
}

export const ACCESSORIES_BY_LOOK: Record<string, readonly AccessorySpec[]> = {
  original: [],
  haru: [
    { kind: 'scarf', color: '#3b78c8', accent: '#d9e8f7' },
    { kind: 'collar', color: '#1b1b21', accent: '#f4f4f4' },
  ],
  hiyori: [{ kind: 'ribbons', color: '#d63a3a' }],
  mao: [{ kind: 'witch-hat', color: '#2a2e66', accent: '#111433' }],
  mark: [{ kind: 'hood', color: '#ff5c5c' }],
  rice: [{ kind: 'beret', color: '#4f6fa8', accent: '#1c1c22' }],
}

const ACCESSORY_ROOT = 'mika-accessories'

function toon(color: string): THREE.Material {
  return new THREE.MeshToonMaterial({ color: new THREE.Color(color) })
}

function build(spec: AccessorySpec): THREE.Object3D {
  const group = new THREE.Group()
  switch (spec.kind) {
    case 'witch-hat': {
      const cone = new THREE.Mesh(new THREE.ConeGeometry(0.11, 0.26, 24), toon(spec.color))
      cone.position.y = 0.33
      cone.rotation.z = -0.12
      const brim = new THREE.Mesh(new THREE.TorusGeometry(0.15, 0.035, 12, 32), toon(spec.color))
      brim.rotation.x = Math.PI / 2
      brim.position.y = 0.21
      const band = new THREE.Mesh(new THREE.CylinderGeometry(0.105, 0.115, 0.03, 24), toon(spec.accent ?? '#000'))
      band.position.y = 0.23
      group.add(cone, brim, band)
      break
    }
    case 'beret': {
      const cap = new THREE.Mesh(
        new THREE.SphereGeometry(0.15, 24, 16, 0, Math.PI * 2, 0, Math.PI / 2),
        toon(spec.color),
      )
      cap.scale.set(1.1, 0.55, 1.15)
      cap.position.set(0.02, 0.2, -0.02)
      cap.rotation.z = -0.25
      const bow = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.02, 0.03), toon(spec.accent ?? '#000'))
      bow.position.set(0.11, 0.23, 0.02)
      group.add(cap, bow)
      break
    }
    case 'scarf': {
      const loop = new THREE.Mesh(new THREE.TorusGeometry(0.075, 0.03, 12, 32), toon(spec.color))
      loop.rotation.x = Math.PI / 2
      loop.position.y = 0.02
      const stripe = new THREE.Mesh(new THREE.TorusGeometry(0.076, 0.012, 8, 32), toon(spec.accent ?? '#fff'))
      stripe.rotation.x = Math.PI / 2
      stripe.position.y = 0.035
      const tail = new THREE.Mesh(new THREE.BoxGeometry(0.05, 0.14, 0.03), toon(spec.color))
      tail.position.set(0.04, -0.07, 0.08)
      tail.rotation.z = 0.2
      group.add(loop, stripe, tail)
      break
    }
    case 'ribbons': {
      for (const side of [-1, 1]) {
        const bow = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.035, 0.02), toon(spec.color))
        bow.position.set(side * 0.1, 0.19, 0.0)
        bow.rotation.z = side * 0.4
        const tie = new THREE.Mesh(new THREE.BoxGeometry(0.015, 0.09, 0.015), toon(spec.color))
        tie.position.set(side * 0.11, 0.13, 0.0)
        group.add(bow, tie)
      }
      break
    }
    case 'hood': {
      const hood = new THREE.Mesh(new THREE.SphereGeometry(0.16, 24, 16, Math.PI, Math.PI), toon(spec.color))
      hood.scale.set(1.05, 1.1, 1)
      hood.position.set(0, 0.12, -0.03)
      group.add(hood)
      break
    }
    case 'collar': {
      for (const side of [-1, 1]) {
        const lapel = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.12, 0.012), toon(spec.color))
        lapel.position.set(side * 0.055, -0.05, 0.085)
        lapel.rotation.z = side * -0.35
        group.add(lapel)
      }
      const shirt = new THREE.Mesh(new THREE.BoxGeometry(0.05, 0.1, 0.01), toon(spec.accent ?? '#fff'))
      shirt.position.set(0, -0.06, 0.08)
      group.add(shirt)
      break
    }
  }
  return group
}

const BONE_BY_KIND: Record<AccessoryKind, 'head' | 'neck' | 'chest'> = {
  'witch-hat': 'head',
  beret: 'head',
  ribbons: 'head',
  hood: 'head',
  scarf: 'neck',
  collar: 'chest',
}

export interface MountedAccessory {
  object: THREE.Object3D
  bone: THREE.Object3D
}

// Pieces stay in the scene and follow the NORMALIZED bone each frame (identity rest pose); the raw
// VRoid bone carries its own rotations, and parenting to it put the hat in front of the face.
export function mountAccessories(vrm: VRM, specs: readonly AccessorySpec[]): MountedAccessory[] {
  unmountAccessories(vrm)
  const mounted: MountedAccessory[] = []
  for (const spec of specs) {
    const bone = vrm.humanoid?.getNormalizedBoneNode(BONE_BY_KIND[spec.kind])
    if (!bone) continue
    const object = build(spec)
    object.name = ACCESSORY_ROOT
    vrm.scene.add(object)
    mounted.push({ object, bone })
  }
  return mounted
}

const worldPosition = new THREE.Vector3()
const worldQuaternion = new THREE.Quaternion()

// Call after vrm.update(): copies the bone's world position/rotation to the piece.
export function syncAccessories(mounted: readonly MountedAccessory[]): void {
  for (const { object, bone } of mounted) {
    bone.getWorldPosition(worldPosition)
    bone.getWorldQuaternion(worldQuaternion)
    object.position.copy(worldPosition)
    object.quaternion.copy(worldQuaternion)
  }
}

export function unmountAccessories(vrm: VRM): void {
  const stale: THREE.Object3D[] = []
  vrm.scene.traverse((object) => {
    if (object.name === ACCESSORY_ROOT) stale.push(object)
  })
  for (const object of stale) {
    object.parent?.remove(object)
    object.traverse((child) => {
      const mesh = child as THREE.Mesh
      if (!mesh.isMesh) return
      mesh.geometry.dispose()
      for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) material.dispose()
    })
  }
}
