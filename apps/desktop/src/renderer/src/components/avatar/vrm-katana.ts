import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { HiltGrip, type HoldWeights } from './vrm-hilt-grip'
import {
  type BladePose,
  bladePoseInto,
  PETAL_CAPACITY,
  PetalSwarm,
  type Point3,
  REDUCED_PETALS,
  SENBONZAKURA_PETALS,
  SENBONZAKURA_TIMELINE,
} from './vrm-senbonzakura'

/**
 * Senbonzakura: she draws a katana, raises it upright before her face, holds, turns the blade down,
 * lets go — "Chire" — and the blade scatters into a thousand glowing petals that spiral around her
 * and fly off. Every mesh is procedural three.js geometry, built when the move starts and disposed
 * when the last petal is gone. The hilt sits in her palm frame, read from each rig's own hand bones
 * (`vrm-hilt-grip.ts`); the timeline and the swarm math live in `vrm-senbonzakura.ts`.
 */
export const SCATTER_AT = SENBONZAKURA_TIMELINE.scatterAt
const ACTION = 'senbonzakura'
const BLADE_LENGTH = 0.72
const HILT_TOP = 0.13
const PETAL_PINK = 0xff8fc1

const UP = new THREE.Vector3(0, 1, 0)

function standard(color: number, metalness: number, roughness: number): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, metalness, roughness })
}

/** Additive and outside tone mapping: the glow adds light, so the anime post-fx blooms it. */
function glow(color: number, opacity: number): THREE.MeshBasicMaterial {
  return new THREE.MeshBasicMaterial({
    color,
    transparent: true,
    opacity,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    toneMapped: false,
    side: THREE.DoubleSide,
  })
}

/** Wrapped hilt with its diamond binding, a tsuba, and a blade with a glowing edge (a child, so it can vanish alone). */
function buildKatana(): { root: THREE.Group; blade: THREE.Group } {
  const root = new THREE.Group()
  const handle = new THREE.Mesh(new THREE.CylinderGeometry(0.014, 0.013, 0.24, 10), standard(0x1b1b2a, 0.1, 0.8))
  const wrap = new THREE.Mesh(new THREE.TorusGeometry(0.015, 0.003, 4, 10), standard(0xe9e2d0, 0, 0.9))
  const wraps = new THREE.InstancedMesh(wrap.geometry, wrap.material, 7)
  for (let i = 0; i < 7; i++) {
    const m = new THREE.Matrix4().compose(
      new THREE.Vector3(0, -0.1 + i * 0.033, 0),
      new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI / 2, 0, (i % 2 ? 1 : -1) * 0.5)),
      new THREE.Vector3(1, 1, 1),
    )
    wraps.setMatrixAt(i, m)
  }
  const tsuba = new THREE.Mesh(new THREE.CylinderGeometry(0.038, 0.038, 0.008, 18), standard(0xc9a13b, 0.8, 0.35))
  tsuba.position.y = 0.125
  const blade = new THREE.Group()
  const steel = new THREE.MeshStandardMaterial({ color: 0xe8eef6, metalness: 0.95, roughness: 0.18 })
  const body = new THREE.Mesh(new THREE.BoxGeometry(0.026, BLADE_LENGTH, 0.005), steel)
  body.position.y = HILT_TOP + BLADE_LENGTH / 2
  const edge = new THREE.Mesh(new THREE.BoxGeometry(0.006, BLADE_LENGTH, 0.007), glow(PETAL_PINK, 0.85))
  edge.position.set(0.012, HILT_TOP + BLADE_LENGTH / 2, 0)
  const tip = new THREE.Mesh(new THREE.ConeGeometry(0.0185, 0.05, 4), steel)
  tip.position.y = HILT_TOP + BLADE_LENGTH + 0.025
  tip.rotation.y = Math.PI / 4
  blade.add(body, edge, tip)
  root.add(handle, wraps, tsuba, blade)
  return { root, blade }
}

/** A cherry petal: rounded at the tip with the little notch, pointed at the base. */
function petalGeometry(): THREE.ShapeGeometry {
  const s = new THREE.Shape()
  s.moveTo(0, -0.012)
  s.quadraticCurveTo(0.014, -0.002, 0.009, 0.01)
  s.lineTo(0, 0.006)
  s.lineTo(-0.009, 0.01)
  s.quadraticCurveTo(-0.014, -0.002, 0, -0.012)
  return new THREE.ShapeGeometry(s, 3)
}

function disposeTree(root: THREE.Object3D): void {
  root.parent?.remove(root)
  root.traverse((child) => {
    const mesh = child as THREE.Mesh
    if (!mesh.isMesh) return
    mesh.geometry.dispose()
    for (const m of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) m.dispose()
  })
}

const dummy = new THREE.Object3D()
const bladePoint = new THREE.Vector3()

interface Built {
  katana: { root: THREE.Group; blade: THREE.Group }
  petals: THREE.InstancedMesh
}

export class VrmKatana {
  private readonly grip = new HiltGrip()
  private held = false
  private built: Built | null = null
  private readonly swarm = new PetalSwarm()
  private readonly blade: BladePose = { hilt: 0, raise: 0, join: 0, flip: 0, bladeVisible: false }
  private readonly weights: HoldWeights = { hold: 0, raise: 0, join: 0, release: 0 }
  private scattered = false
  private reduced = false

  constructor(
    private readonly scene: THREE.Scene,
    private readonly onScatter: () => void,
  ) {}

  attach(vrm: VRM): void {
    this.grip.attach(vrm)
  }

  /**
   * Call after the body pose is written and before `vrm.update()`: puts the hilt in her palm and
   * bends her arms and fingers onto it on the normalized rig, so the mesh shows the grip this frame.
   */
  hold(action: string | undefined, progress: number): void {
    const blade = bladePoseInto(this.blade, action === ACTION ? progress : 0)
    const w = this.weights
    w.hold = blade.hilt
    w.raise = blade.raise
    w.join = blade.join
    w.release = blade.flip
    this.held = this.grip.solve(w)
  }

  /** Reduced motion: the move still plays, with a handful of petals and no full-screen storm. */
  setReducedMotion(reduced: boolean): void {
    this.reduced = reduced
  }

  get bladeVisible(): boolean {
    const katana = this.built?.katana
    return Boolean(katana?.root.visible && katana.blade.visible)
  }

  get visiblePetals(): number {
    return this.built?.petals.count ?? 0
  }

  bladeDirection(): THREE.Vector3 {
    return UP.clone().applyQuaternion(this.built?.katana.root.quaternion ?? new THREE.Quaternion())
  }

  /** Call after `vrm.update()`: builds, poses and disposes the meshes; `wind` carries the petals. */
  update(dt: number, action: string | undefined, progress: number, wind?: Readonly<Point3>): void {
    const playing = action === ACTION && progress > 0 && progress < 1 && this.grip.attached
    if (!playing) this.scattered = false
    if (!playing && !this.swarm.alive) {
      this.teardown()
      return
    }
    const built = this.build()
    this.pose(built, playing ? progress : 1)
    if (playing && progress >= SCATTER_AT && !this.scattered) this.scatter(built)
    this.swarm.step(dt, wind)
    this.writePetals(built.petals)
  }

  private build(): Built {
    if (this.built) return this.built
    const petals = new THREE.InstancedMesh(petalGeometry(), glow(PETAL_PINK, 0.9), PETAL_CAPACITY)
    petals.frustumCulled = false
    petals.count = 0
    const built = { katana: buildKatana(), petals }
    this.scene.add(built.katana.root, petals)
    this.built = built
    return built
  }

  private teardown(): void {
    if (!this.built) return
    disposeTree(this.built.katana.root)
    disposeTree(this.built.petals)
    this.built = null
  }

  private pose({ katana }: Built, progress: number): void {
    const blade = bladePoseInto(this.blade, progress)
    const root = katana.root
    root.visible = blade.hilt > 0.01 && this.held
    if (!root.visible) return
    root.position.copy(this.grip.position)
    root.quaternion.copy(this.grip.quaternion)
    root.scale.setScalar(blade.hilt)
    katana.blade.visible = blade.bladeVisible
  }

  /** "Chire": every petal is born somewhere along the blade that just dissolved. */
  private scatter({ katana }: Built): void {
    this.scattered = true
    katana.root.updateMatrixWorld(true)
    const count = this.reduced ? REDUCED_PETALS : SENBONZAKURA_PETALS
    this.swarm.spawn(count, (i: number, out: Point3) => this.bladeOrigin(katana.root, i / count, out), {
      x: 0,
      z: 0,
    })
    if (!this.reduced) this.onScatter()
  }

  private bladeOrigin(root: THREE.Object3D, along: number, out: Point3): void {
    bladePoint.set(0, HILT_TOP + along * BLADE_LENGTH, 0)
    root.localToWorld(bladePoint)
    out.x = bladePoint.x
    out.y = bladePoint.y
    out.z = bladePoint.z
  }

  private writePetals(petals: THREE.InstancedMesh): void {
    const s = this.swarm
    for (let i = 0; i < s.count; i++) {
      dummy.position.set(s.x[i], s.y[i], s.z[i])
      dummy.rotation.set(s.rx[i], s.ry[i], s.rz[i])
      dummy.scale.setScalar(s.scale[i])
      dummy.updateMatrix()
      petals.setMatrixAt(i, dummy.matrix)
    }
    petals.count = s.count
    if (s.count > 0) petals.instanceMatrix.needsUpdate = true
  }

  dispose(): void {
    this.teardown()
  }
}
