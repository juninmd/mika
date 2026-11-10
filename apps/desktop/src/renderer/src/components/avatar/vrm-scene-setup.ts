import { type VRM, VRMLoaderPlugin, VRMUtils } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { type GLTF, GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { MAX_FRAME_DELTA_SECONDS } from '../live2d/live2d-physics'
import { type SpringState, springTo } from './damped-spring'
import type { BlinkDriver } from './vrm-blink'
import type { VrmLifeSnapshot } from './vrm-debug-bridge'
import { tightenEyeRange } from './vrm-gaze'
import { actionFaceWeight, meshVisibleInLook, tintFor, VRM_PRESETS, type VrmLook } from './vrm-look'
import { forceImgTextureLoader, type TextureLoaderHost } from './vrm-texture-loader'

type MToonLike = THREE.Material & {
  color?: THREE.Color
  shadeColorFactor?: THREE.Color
  map?: THREE.Texture | null
  shadeMultiplyTexture?: THREE.Texture | null
}

type ExpressionManager = NonNullable<VRM['expressionManager']>

/**
 * `vrm.update(dt)`, except the spring bones take the frame in substeps of ≤ 1/30 s: one 50 ms
 * Verlet step at low fps (or after a hitch) makes hair and skirt jump and jitter. The rest (pose
 * copy, look-at, expressions, node constraints, materials) runs once, as `VRM.update` orders it.
 */
export function stepVrm(vrm: VRM, dt: number): void {
  vrm.humanoid?.update()
  vrm.lookAt?.update(dt)
  vrm.expressionManager?.update()
  vrm.nodeConstraintManager?.update()
  // `frameSubsteps`' split, inlined: its result object would be one allocation per frame.
  const steps = dt > 0 ? Math.ceil(dt / MAX_FRAME_DELTA_SECONDS - 1e-9) : 0
  for (let i = 0; i < steps; i++) vrm.springBoneManager?.update(dt / steps)
  if (vrm.materials) for (const material of vrm.materials) (material as { update?: (d: number) => void }).update?.(dt)
}

export function addDefaultLights(scene: THREE.Scene): void {
  scene.add(new THREE.AmbientLight(0xffffff, 0.9))
  const key = new THREE.DirectionalLight(0xffffff, 1.2)
  key.position.set(1, 2, 2)
  scene.add(key)
}

// Full-body framing from the loaded scene's own bounds (hair/hat to feet, arm span included): the
// companion window is fixed-size, so anything outside the frustum is cut by the window edge.
export function fitCamera(camera: THREE.PerspectiveCamera, width: number, height: number, vrm?: VRM | null): void {
  camera.aspect = width / Math.max(1, height)
  vrm?.scene.updateMatrixWorld(true)
  const bounds = vrm ? new THREE.Box3().setFromObject(vrm.scene) : null
  if (!bounds || bounds.isEmpty()) {
    const portrait = camera.aspect < 0.8
    camera.position.set(0, 0.85, portrait ? 3.6 : 4.2)
    camera.lookAt(0, 0.85, 0)
    camera.updateProjectionMatrix()
    return
  }
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  const tanV = Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2)
  const frameMargin = 1.08 // breathing room so hair and shoes never touch the edge
  const fitHeight = size.y / (2 * tanV)
  const fitWidth = size.x / (2 * tanV * camera.aspect)
  // Measured from the model's front face, so its nearest point is what must fit.
  const distance = Math.max(fitHeight, fitWidth) * frameMargin + size.z / 2
  camera.position.set(center.x, center.y, center.z + distance)
  camera.lookAt(center)
  camera.near = Math.max(0.05, distance - size.z - 1)
  camera.far = distance + size.z + 5
  camera.updateProjectionMatrix()
}

export function attachEyeTarget(vrm: VRM, eyeTarget: THREE.Object3D): void {
  if (vrm.lookAt) {
    vrm.lookAt.target = eyeTarget
    tightenEyeRange(vrm.lookAt.applier)
  }
}

export function disposeVrmScene(scene: THREE.Scene, vrm: VRM): void {
  scene.remove(vrm.scene)
  VRMUtils.deepDispose(vrm.scene)
}

export async function loadVrmGltf(url: string): Promise<{ gltf: GLTF; vrm: VRM }> {
  const loader = new GLTFLoader()
  // Must register before the VRM plugin, or embedded images never reach the material (vrm-texture-loader.ts).
  loader.register((parser) => {
    forceImgTextureLoader(parser as unknown as TextureLoaderHost)
    return { name: 'mika-img-texture-loader' }
  })
  loader.register((parser) => new VRMLoaderPlugin(parser))
  const gltf = await loader.loadAsync(url)
  const vrm = gltf.userData.vrm as VRM
  VRMUtils.removeUnnecessaryVertices(gltf.scene)
  VRMUtils.combineSkeletons(gltf.scene)
  VRMUtils.rotateVRM0(vrm)
  return { gltf, vrm }
}

export function applyLookTints(root: THREE.Object3D | undefined, look: VrmLook): void {
  root?.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    mesh.visible = meshVisibleInLook(mesh.name, look)
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
    for (const material of materials as MToonLike[]) {
      const tint = tintFor(material.name, look)
      if (!tint) continue
      material.color?.set(tint.color)
      if (tint.shade) material.shadeColorFactor?.set(tint.shade)
      if (tint.flat) {
        material.map = null
        material.shadeMultiplyTexture = null
      }
      material.needsUpdate = true
    }
  })
}

/** Rigidez da mola das expressões: ~0.3 s de transição, com entrada e saída suaves. */
const PRESET_OMEGA = 16
/** Reaproveitado a cada preset: zero alocação por quadro. */
const presetSpring: SpringState = { value: 0, velocity: 0 }

export function blendPresets(
  em: ExpressionManager,
  expression: string,
  expressionIntensity: number,
  actionPreset: string | null,
  actionName: string,
  pulse: number,
  dt: number,
  velocity: Record<string, number>,
): void {
  for (const preset of VRM_PRESETS) {
    const base = preset === expression ? expressionIntensity : 0
    const action = preset === actionPreset ? pulse * actionFaceWeight(actionName) : 0
    presetSpring.value = em.getValue(preset) ?? 0
    presetSpring.velocity = velocity[preset] ?? 0
    // Clamp: a mola não passa do alvo, mas soma de ponto flutuante não pode sair de [0, 1].
    em.setValue(preset, Math.min(1, Math.max(0, springTo(presetSpring, Math.max(base, action), PRESET_OMEGA, dt))))
    velocity[preset] = presetSpring.velocity
  }
}

export function writeVisemes(em: ExpressionManager, visemes: Record<string, number>): void {
  for (const [preset, value] of Object.entries(visemes)) em.setValue(preset, value)
}

export function writeBlink(
  em: ExpressionManager,
  blink: BlinkDriver,
  winking: boolean,
  dt: number,
  gazeSpeed = 0,
  speaking = false,
): void {
  const eyesBusy = Math.max(em.getValue('happy') ?? 0, em.getValue('relaxed') ?? 0)
  em.setValue('blink', blink.advance(dt, winking ? 1 : eyesBusy, gazeSpeed, speaking))
}

export function readVrmLife(vrm: VRM | null, renderer: THREE.WebGLRenderer, postFx: boolean): VrmLifeSnapshot {
  const humanoid = vrm?.humanoid
  const em = vrm?.expressionManager
  let springPose = 0
  for (const joint of vrm?.springBoneManager?.joints ?? []) {
    springPose += Math.abs(joint.bone.quaternion.x) + Math.abs(joint.bone.quaternion.z)
  }
  const visemes = ['aa', 'ih', 'ou', 'ee', 'oh'].map((name) => em?.getValue(name) ?? 0)
  return {
    headYaw: humanoid?.getNormalizedBoneNode('head')?.rotation.y ?? 0,
    headPitch: humanoid?.getNormalizedBoneNode('head')?.rotation.x ?? 0,
    neckYaw: humanoid?.getNormalizedBoneNode('neck')?.rotation.y ?? 0,
    blink: em?.getValue('blink') ?? 0,
    happy: em?.getValue('happy') ?? 0,
    mouth: Math.max(...visemes),
    postFx,
    springPose,
    springFinite: Number.isFinite(springPose),
    textures: renderer.info.memory.textures,
    geometries: renderer.info.memory.geometries,
    canvasWidth: renderer.domElement.width,
  }
}
