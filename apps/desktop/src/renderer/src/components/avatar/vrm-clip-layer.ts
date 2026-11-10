import type { VRM } from '@pixiv/three-vrm'
import {
  createVRMAnimationHumanoidTracks,
  type VRMAnimation,
  VRMAnimationLoaderPlugin,
} from '@pixiv/three-vrm-animation'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

/**
 * Motion-captured `.vrma` clips blended over the procedural pose. A procedural rig alone reads as a
 * mannequin: it lacks the weight shifts, settling and whole-body phrasing of captured motion. The
 * clip only owns the skeleton; face, gaze and the additive ambient layers keep running on top.
 */
export class VrmClipLayer {
  private mixer?: THREE.AnimationMixer
  private readonly clips = new Map<string, THREE.AnimationClip>()
  private current?: THREE.AnimationAction
  private currentName?: string
  private weight = 0
  private target = 0
  private fadeRate = 3
  private bones: THREE.Object3D[] = []
  private readonly held = new Map<THREE.Object3D, THREE.Quaternion>()
  private readonly hipsRest = new THREE.Vector3()
  private readonly hipsBase = new THREE.Vector3()
  private hips?: THREE.Object3D

  async attach(vrm: VRM, urls: Record<string, string>): Promise<void> {
    this.mixer = new THREE.AnimationMixer(vrm.scene)
    this.hips = vrm.humanoid.getNormalizedBoneNode('hips') ?? undefined
    if (this.hips) this.hipsRest.copy(this.hips.position)
    const loader = new GLTFLoader()
    loader.register((parser) => new VRMAnimationLoaderPlugin(parser))
    const entries = await Promise.allSettled(
      Object.entries(urls).map(async ([name, url]) => {
        const gltf = await loader.loadAsync(url)
        const animation = (gltf.userData.vrmAnimations as VRMAnimation[] | undefined)?.[0]
        if (animation) this.clips.set(name, humanoidClip(name, animation, vrm))
      }),
    )
    const tracked = new Set<THREE.Object3D>()
    for (const clip of this.clips.values()) {
      for (const track of clip.tracks) {
        const node = vrm.scene.getObjectByName(track.name.split('.')[0] ?? '')
        if (node) tracked.add(node)
      }
    }
    this.bones = [...tracked]
    // One scratch quaternion per bone, reused every frame by `blend`.
    for (const bone of this.bones) this.held.set(bone, new THREE.Quaternion())
    for (const entry of entries) if (entry.status === 'rejected') throw entry.reason
  }

  has(name: string): boolean {
    return this.clips.has(name)
  }

  get playing(): string | undefined {
    return this.target > 0 ? this.currentName : undefined
  }

  /** Remaining seconds of a one-shot clip; loops report Infinity. */
  get remaining(): number {
    const action = this.current
    if (!action || this.target === 0) return 0
    if (action.loop === THREE.LoopRepeat) return Number.POSITIVE_INFINITY
    return Math.max(0, action.getClip().duration / action.timeScale - action.time / action.timeScale)
  }

  play(name: string, { loop = false, fade = 0.4, speed = 1 } = {}): boolean {
    const clip = this.clips.get(name)
    if (!clip || !this.mixer) return false
    const next = this.mixer.clipAction(clip)
    next.reset()
    next.setLoop(loop ? THREE.LoopRepeat : THREE.LoopOnce, loop ? Number.POSITIVE_INFINITY : 1)
    next.clampWhenFinished = true
    next.timeScale = speed
    next.play()
    if (this.current && this.current !== next) next.crossFadeFrom(this.current, fade, false)
    this.current = next
    this.currentName = name
    this.target = 1
    this.fadeRate = 1 / Math.max(0.05, fade)
    return true
  }

  stop(fade = 0.5): void {
    this.target = 0
    this.fadeRate = 1 / Math.max(0.05, fade)
  }

  /** Call after the procedural pose is written: blends the clip over it by the current weight. */
  blend(dt: number): void {
    const mixer = this.mixer
    if (!mixer || !this.current) return
    if (this.remaining <= 0.35 && this.current.loop !== THREE.LoopRepeat) this.target = 0
    this.weight +=
      Math.sign(this.target - this.weight) * Math.min(Math.abs(this.target - this.weight), dt * this.fadeRate)
    if (this.weight <= 0) {
      if (this.target === 0) this.current.stop()
      return
    }
    for (const bone of this.bones) this.held.get(bone)?.copy(bone.quaternion)
    const hipsBase = this.hipsBase.copy(this.hipsRest)
    mixer.update(dt)
    const w = smooth(this.weight)
    for (const bone of this.bones) {
      const base = this.held.get(bone)
      if (base) bone.quaternion.copy(base.slerp(bone.quaternion, w))
    }
    if (this.hips) this.hips.position.lerpVectors(hipsBase, this.hips.position, w)
  }

  /** Resets the hips the clip may have translated, so the procedural frame starts from rest. */
  resetRoot(): void {
    this.hips?.position.copy(this.hipsRest)
  }
}

const smooth = (t: number) => t * t * (3 - 2 * t)

function humanoidClip(name: string, animation: VRMAnimation, vrm: VRM): THREE.AnimationClip {
  const { rotation, translation } = createVRMAnimationHumanoidTracks(animation, vrm.humanoid, vrm.meta.metaVersion)
  const tracks: THREE.KeyframeTrack[] = [...rotation.values(), ...translation.values()]
  return new THREE.AnimationClip(name, animation.duration, tracks)
}
