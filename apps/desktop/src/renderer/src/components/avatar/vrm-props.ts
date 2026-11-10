import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { drinkSip } from './vrm-body-rig'

/** Action progress at which the throw lets go of the tomato. */
const RELEASE = 0.44
const FLIGHT_SEC = 0.45
const SPLAT_SEC = 3.4
/** Depth in front of the lens where the tomato lands, inside the view but past the near plane. */
const SPLAT_DEPTH = 0.32

interface Grip {
  offset: THREE.Vector3
  rotation: THREE.Quaternion
}

// Normalized hand space: identity at rest, fingers along -X for the right hand, palm facing -Y.
const BOTTLE_GRIP: Grip = {
  offset: new THREE.Vector3(-0.06, -0.025, 0.01),
  rotation: new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI / 2, 0, 0)),
}
/** Lips in normalized head space, where the bottle's neck is aimed while she drinks. */
const MOUTH = new THREE.Vector3(0, 0.05, 0.08)
const UP = new THREE.Vector3(0, 1, 0)
const TOMATO_GRIP: Grip = { offset: new THREE.Vector3(-0.065, -0.03, 0), rotation: new THREE.Quaternion() }

const toon = (color: string, opacity = 1) =>
  new THREE.MeshToonMaterial({ color: new THREE.Color(color), transparent: opacity < 1, opacity })

function buildBottle(): THREE.Group {
  const bottle = new THREE.Group()
  const shell = new THREE.Mesh(new THREE.CylinderGeometry(0.021, 0.021, 0.13, 20), toon('#cfeaff', 0.55))
  const water = new THREE.Mesh(new THREE.CylinderGeometry(0.018, 0.018, 0.09, 20), toon('#4fb4f0', 0.85))
  water.position.y = -0.018
  const label = new THREE.Mesh(new THREE.CylinderGeometry(0.0215, 0.0215, 0.035, 20, 1, true), toon('#f6a6c6'))
  label.position.y = -0.01
  const neck = new THREE.Mesh(new THREE.CylinderGeometry(0.009, 0.019, 0.022, 16), toon('#cfeaff', 0.55))
  neck.position.y = 0.076
  const cap = new THREE.Mesh(new THREE.CylinderGeometry(0.0105, 0.0105, 0.014, 16), toon('#ffffff'))
  cap.position.y = 0.093
  bottle.add(shell, water, label, neck, cap)
  return bottle
}

function buildTomato(): THREE.Group {
  const tomato = new THREE.Group()
  const fruit = new THREE.Mesh(new THREE.SphereGeometry(0.032, 24, 16), toon('#e8322b'))
  fruit.scale.set(1, 0.84, 1)
  tomato.add(fruit)
  for (let i = 0; i < 5; i++) {
    const leaf = new THREE.Mesh(new THREE.BoxGeometry(0.022, 0.003, 0.007), toon('#3f9a3a'))
    const angle = (i / 5) * Math.PI * 2
    leaf.position.set(Math.cos(angle) * 0.01, 0.027, Math.sin(angle) * 0.01)
    leaf.rotation.set(0, -angle, 0.25)
    tomato.add(leaf)
  }
  const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.0025, 0.003, 0.012, 8), toon('#2f7a2c'))
  stem.position.y = 0.033
  tomato.add(stem)
  return tomato
}

/** Juice splash drawn once: a lumpy core, flung droplets, drips running down and a few seeds. */
function splatTexture(): THREE.CanvasTexture {
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = 512
  const g = canvas.getContext('2d') as CanvasRenderingContext2D
  const blob = (x: number, y: number, r: number, color: string) => {
    g.fillStyle = color
    g.beginPath()
    g.arc(x, y, r, 0, Math.PI * 2)
    g.fill()
  }
  for (let i = 0; i < 26; i++) {
    const a = (i / 26) * Math.PI * 2 + Math.sin(i * 7.3) * 0.2
    const d = 60 + (Math.sin(i * 12.9) * 0.5 + 0.5) * 150
    blob(256 + Math.cos(a) * d, 230 + Math.sin(a) * d * 0.9, 6 + (i % 4) * 5, '#c9160f')
  }
  for (let i = 0; i < 9; i++) {
    const a = (i / 9) * Math.PI * 2
    blob(256 + Math.cos(a) * 55, 230 + Math.sin(a) * 50, 62 + (i % 3) * 12, '#d81d14')
  }
  blob(256, 230, 95, '#e2261a')
  for (const [x, length] of [
    [190, 170],
    [240, 250],
    [300, 205],
    [335, 130],
  ]) {
    g.fillStyle = '#d81d14'
    g.fillRect(x - 9, 250, 18, length)
    blob(x, 250 + length, 13, '#d81d14')
  }
  for (let i = 0; i < 11; i++) {
    g.fillStyle = '#ffd86b'
    g.beginPath()
    g.ellipse(200 + ((i * 47) % 120), 190 + ((i * 29) % 90), 7, 4, i, 0, Math.PI * 2)
    g.fill()
  }
  blob(215, 185, 22, 'rgba(255,255,255,0.45)')
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

const easeOutBack = (t: number) => 1 + 2.7 * (t - 1) ** 3 + 1.7 * (t - 1) ** 2
/** Anime prop pop: grows out of the hand, shrinks away before the pose releases. */
function presence(progress: number): number {
  const grow = Math.min(1, Math.max(0, (progress - 0.04) / 0.12))
  const shrink = Math.min(1, Math.max(0, (0.96 - progress) / 0.1))
  return Math.max(0, Math.min(easeOutBack(grow), shrink))
}

const handPosition = new THREE.Vector3()
const handQuaternion = new THREE.Quaternion()

/** Props the body actions hold or throw: a water bottle for `drink`, a tomato for `throw-tomato`. */
export class VrmActionProps {
  private hand?: THREE.Object3D
  private head?: THREE.Object3D
  private readonly bottle = buildBottle()
  private readonly tomato = buildTomato()
  private splat?: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
  private flight = -1
  private launched = false
  private splatAge = -1
  private readonly from = new THREE.Vector3()
  private readonly landing = new THREE.Vector3()

  constructor(
    scene: THREE.Scene,
    private readonly camera: THREE.Camera,
  ) {
    // The splat rides the camera, which only renders its children once it is in the scene.
    scene.add(this.bottle, this.tomato, camera)
    this.bottle.visible = this.tomato.visible = false
  }

  attach(vrm: VRM): void {
    this.hand = vrm.humanoid?.getNormalizedBoneNode('rightHand') ?? undefined
    this.head = vrm.humanoid?.getNormalizedBoneNode('head') ?? undefined
  }

  /** Call after `vrm.update()`, so the hand's world transform is current. */
  update(dt: number, action: string | undefined, progress: number): void {
    const throwing = action === 'throw-tomato' && progress < 1
    if (throwing && progress >= RELEASE && !this.launched) this.launch()
    if (!throwing || progress < RELEASE) this.launched = false
    this.hold(this.bottle, action === 'drink' && progress < 1 ? presence(progress) : 0, BOTTLE_GRIP)
    if (this.bottle.visible) this.aimAtLips(drinkSip(progress))
    if (this.flight < 0) this.hold(this.tomato, throwing && progress < RELEASE ? presence(progress) : 0, TOMATO_GRIP)
    this.fly(dt)
    this.drip(dt)
  }

  private hold(prop: THREE.Object3D, scale: number, grip: Grip): void {
    prop.visible = scale > 0.01 && Boolean(this.hand)
    if (!prop.visible || !this.hand) return
    this.hand.getWorldPosition(handPosition)
    this.hand.getWorldQuaternion(handQuaternion)
    prop.position.copy(grip.offset).applyQuaternion(handQuaternion).add(handPosition)
    prop.quaternion.copy(handQuaternion).multiply(grip.rotation)
    prop.scale.setScalar(scale)
  }

  /** Near the face the fist turns the bottle so its neck meets the lips; far away it stays in the grip. */
  private aimAtLips(sip: number): void {
    if (!this.head) return
    this.head.updateWorldMatrix(true, false)
    const mouth = this.head.localToWorld(MOUTH.clone())
    const fist = this.bottle.position.clone()
    const toMouth = mouth.sub(fist)
    const near = 1 - THREE.MathUtils.smoothstep(toMouth.length(), 0.15, 0.35)
    if (near <= 0) return
    const axis = toMouth.normalize()
    axis.y -= sip * 0.35
    axis.normalize()
    const aimed = new THREE.Quaternion().setFromUnitVectors(UP, axis)
    this.bottle.quaternion.slerp(aimed, near)
    this.bottle.position.addScaledVector(axis, 0.02 * near)
  }

  private launch(): void {
    this.launched = true
    this.flight = 0
    this.from.copy(this.tomato.position)
    const x = (Math.random() - 0.5) * 0.06
    const y = (Math.random() - 0.3) * 0.05
    this.landing.set(x, y, -SPLAT_DEPTH)
  }

  private fly(dt: number): void {
    if (this.flight < 0) return
    this.flight += dt
    const t = Math.min(1, this.flight / FLIGHT_SEC)
    const target = this.camera.localToWorld(this.landing.clone())
    this.tomato.visible = true
    this.tomato.scale.setScalar(1)
    this.tomato.position.lerpVectors(this.from, target, t)
    this.tomato.position.y += Math.sin(t * Math.PI) * 0.1
    this.tomato.rotation.set(t * 9, t * 5, 0)
    if (t < 1) return
    this.flight = -1
    this.tomato.visible = false
    this.impact()
  }

  private impact(): void {
    if (!this.splat) {
      const material = new THREE.MeshBasicMaterial({
        map: splatTexture(),
        transparent: true,
        depthTest: false,
        depthWrite: false,
        toneMapped: false,
      })
      this.splat = new THREE.Mesh(new THREE.PlaneGeometry(0.1, 0.1), material)
      this.splat.renderOrder = 999
      this.camera.add(this.splat)
    }
    this.splat.position.copy(this.landing)
    this.splat.rotation.z = (Math.random() - 0.5) * 0.8
    this.splatAge = 0
  }

  /** Squash on impact, then the juice slides down the glass and fades. */
  private drip(dt: number): void {
    const splat = this.splat
    if (!splat || this.splatAge < 0) return
    this.splatAge += dt
    const age = this.splatAge
    splat.visible = age < SPLAT_SEC
    if (!splat.visible) {
      this.splatAge = -1
      return
    }
    const squash = Math.min(1, age / 0.12)
    splat.scale.set(0.6 + squash * 0.45, (0.6 + squash * 0.4) * (1 + age * 0.06), 1)
    splat.position.y = this.landing.y - age * 0.004
    splat.material.opacity = Math.min(1, (SPLAT_SEC - age) / 0.9)
  }

  dispose(): void {
    for (const root of [this.bottle, this.tomato, this.splat]) {
      root?.parent?.remove(root)
      root?.traverse((child) => {
        const mesh = child as THREE.Mesh
        if (!mesh.isMesh) return
        mesh.geometry.dispose()
        const material = mesh.material as THREE.MeshBasicMaterial
        material.map?.dispose()
        material.dispose()
      })
    }
  }
}
