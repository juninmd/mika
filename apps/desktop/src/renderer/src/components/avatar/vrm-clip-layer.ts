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
  /**
   * What the mixer last wrote per bone. three's PropertyMixer only writes a value that changed, so in a
   * still stretch of a capture it writes nothing; the procedural frame had already reset the bone and
   * the body snapped ("teleported") to rest. Restoring this before `mixer.update` keeps the clip there.
   */
  private readonly written = new Map<THREE.Object3D, THREE.Quaternion>()
  private readonly fades = new Map<THREE.AnimationAction, { from: number; to: number; t: number; span: number }>()
  private readonly hipsWritten = new THREE.Vector3()
  private primed = false
  private readonly hipsRest = new THREE.Vector3()
  private readonly hipsBase = new THREE.Vector3()
  private hips?: THREE.Object3D
  /** Seconds left before a one-shot hands the body back, when the action is shorter than its capture. */
  private holdLeft = Number.POSITIVE_INFINITY
  /** Lazily cloned twin per clip: replaying a clip mid-play crossfades from its twin instead of rewinding in place. */
  private readonly twins = new Map<string, THREE.AnimationClip>()
  /** Hips and legs: the clip can keep them while a procedural gesture owns the rest of the body. */
  private lower = new Set<THREE.Object3D>()
  private upper = 1
  private upperTarget = 1
  private upperRate = 3

  async attach(vrm: VRM, urls: Record<string, string>): Promise<void> {
    const clips = new Map<string, THREE.AnimationClip>()
    const loader = new GLTFLoader()
    loader.register((parser) => new VRMAnimationLoaderPlugin(parser))
    const entries = await Promise.allSettled(
      Object.entries(urls).map(async ([name, url]) => {
        const gltf = await loader.loadAsync(url)
        const animation = (gltf.userData.vrmAnimations as VRMAnimation[] | undefined)?.[0]
        if (animation) clips.set(name, humanoidClip(name, animation, vrm))
      }),
    )
    const lower = new Set<THREE.Object3D>()
    for (const name of LOWER_BODY) {
      const node = vrm.humanoid.getNormalizedBoneNode(name)
      if (node) lower.add(node)
    }
    this.adopt(vrm.scene, vrm.humanoid.getNormalizedBoneNode('hips') ?? undefined, clips, lower)
    for (const entry of entries) if (entry.status === 'rejected') throw entry.reason
  }

  /** Wires already-built clips to the skeleton under `root`. */
  adopt(
    root: THREE.Object3D,
    hips: THREE.Object3D | undefined,
    clips: Map<string, THREE.AnimationClip>,
    lower = new Set<THREE.Object3D>(),
  ): void {
    this.mixer = new THREE.AnimationMixer(root)
    this.fades.clear()
    this.lower = lower
    this.hips = hips
    if (hips) this.hipsRest.copy(hips.position)
    for (const [name, clip] of clips) this.clips.set(name, clip)
    const tracked = new Set<THREE.Object3D>()
    for (const clip of this.clips.values()) {
      for (const track of clip.tracks) {
        const node = root.getObjectByName(track.name.split('.')[0] ?? '')
        if (node) tracked.add(node)
      }
    }
    this.bones = [...tracked]
    // One scratch quaternion per bone, reused every frame by `blend`.
    for (const bone of this.bones) {
      this.held.set(bone, new THREE.Quaternion())
      this.written.set(bone, new THREE.Quaternion())
    }
    this.primed = false
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
    const clipLeft = (action.getClip().duration - action.time) / action.timeScale
    return Math.max(0, Math.min(clipLeft, this.holdLeft))
  }

  /** `hold` caps a one-shot at the action's own length: a 12 s capture must not outlive a 3 s gesture. */
  play(name: string, { loop = false, fade = 0.4, speed = 1, hold = Number.POSITIVE_INFINITY } = {}): boolean {
    const clip = this.clips.get(name)
    if (!clip || !this.mixer) return false
    const live = this.current && this.weight > 0 && this.currentName === name
    if (live && loop) {
      // The loop is still on screen: keep its phase and just fade the layer back up.
      this.target = 1
      this.fadeRate = 1 / Math.max(0.05, fade)
      return true
    }
    const next = this.mixer.clipAction(live && this.current?.getClip() === clip ? this.twin(name, clip) : clip)
    next.reset()
    next.setLoop(loop ? THREE.LoopRepeat : THREE.LoopOnce, loop ? Number.POSITIVE_INFINITY : 1)
    next.clampWhenFinished = true
    next.timeScale = speed
    next.play()
    this.handOver(next, fade)
    this.current = next
    this.currentName = name
    this.target = 1
    this.fadeRate = 1 / Math.max(0.05, fade)
    this.holdLeft = loop ? Number.POSITIVE_INFINITY : hold
    return true
  }

  /**
   * Eased crossfade. three's `crossFadeFrom` ramps linearly (motion starts at full speed and stops dead)
   * and fades the outgoing action from weight 1, so a gesture landing mid-crossfade snapped the idle
   * from 0.1 to full in one frame. Every live action fades from its visible weight instead.
   */
  private handOver(next: THREE.AnimationAction, fade: number): void {
    for (const [action, f] of this.fades) {
      if (action !== next) Object.assign(f, { from: action.weight, to: 0, t: 0, span: fade })
    }
    // A twin replayed while still fading out starts from where it is, not from zero.
    const from = this.fades.has(next) ? next.weight : this.current ? 0 : 1
    this.fades.set(next, { from, to: 1, t: 0, span: fade })
  }

  private advanceFades(dt: number): void {
    for (const [action, f] of this.fades) {
      f.t = Math.min(f.span, f.t + dt)
      action.weight = f.from + (f.to - f.from) * smooth(f.span > 0 ? f.t / f.span : 1)
      if (f.t < f.span || f.to > 0) continue
      action.stop()
      this.fades.delete(action)
    }
  }

  private twin(name: string, clip: THREE.AnimationClip): THREE.AnimationClip {
    let twin = this.twins.get(name)
    if (!twin) {
      twin = clip.clone()
      this.twins.set(name, twin)
    }
    // Alternate: the twin's own replay goes back to the original.
    this.twins.set(name, clip)
    this.clips.set(name, twin)
    return twin
  }

  /** Off: the clip keeps only hips and legs, so a gesture stands on the captured stance. */
  setUpperBody(on: boolean, fade = 0.3): void {
    this.upperTarget = on ? 1 : 0
    this.upperRate = 1 / Math.max(0.05, fade)
  }

  stop(fade = 0.5): void {
    this.target = 0
    this.fadeRate = 1 / Math.max(0.05, fade)
  }

  /** Call after the procedural pose is written: blends the clip over it by the current weight. */
  blend(dt: number): void {
    const mixer = this.mixer
    if (!mixer || !this.current) return
    this.holdLeft -= dt
    if (this.remaining <= 0.35 && this.current.loop !== THREE.LoopRepeat) this.target = 0
    this.weight +=
      Math.sign(this.target - this.weight) * Math.min(Math.abs(this.target - this.weight), dt * this.fadeRate)
    if (this.weight <= 0) {
      if (this.target === 0) this.current.stop()
      return
    }
    const hipsBase = this.hipsBase.copy(this.hipsRest)
    this.sample(mixer, dt)
    this.upper +=
      Math.sign(this.upperTarget - this.upper) * Math.min(Math.abs(this.upperTarget - this.upper), dt * this.upperRate)
    const w = smooth(this.weight)
    const wUpper = w * smooth(this.upper)
    for (const bone of this.bones) {
      const base = this.held.get(bone)
      if (base) bone.quaternion.copy(base.slerp(bone.quaternion, this.lower.has(bone) ? w : wUpper))
    }
    if (this.hips) this.hips.position.lerpVectors(hipsBase, this.hips.position, w)
  }

  /**
   * Runs the mixer on its own last output, not on the procedural pose: the mixer skips a write when
   * the clip value is unchanged, which left the rest pose on screen during still stretches.
   */
  private sample(mixer: THREE.AnimationMixer, dt: number): void {
    // First run: the mixer's change cache starts at the bones' current values, so ours does too.
    if (!this.primed) this.prime()
    for (const bone of this.bones) {
      this.held.get(bone)?.copy(bone.quaternion)
      const written = this.written.get(bone)
      if (written) bone.quaternion.copy(written)
    }
    this.hips?.position.copy(this.hipsWritten)
    this.advanceFades(dt)
    mixer.update(dt)
    for (const bone of this.bones) this.written.get(bone)?.copy(bone.quaternion)
    if (this.hips) this.hipsWritten.copy(this.hips.position)
  }

  private prime(): void {
    for (const bone of this.bones) this.written.get(bone)?.copy(bone.quaternion)
    if (this.hips) this.hipsWritten.copy(this.hips.position)
    this.primed = true
  }

  /** Resets the hips the clip may have translated, so the procedural frame starts from rest. */
  resetRoot(): void {
    this.hips?.position.copy(this.hipsRest)
  }
}

const smooth = (t: number) => t * t * (3 - 2 * t)

const LOWER_BODY = [
  'hips',
  'leftUpperLeg',
  'rightUpperLeg',
  'leftLowerLeg',
  'rightLowerLeg',
  'leftFoot',
  'rightFoot',
  'leftToes',
  'rightToes',
] as const

function humanoidClip(name: string, animation: VRMAnimation, vrm: VRM): THREE.AnimationClip {
  const { rotation, translation } = createVRMAnimationHumanoidTracks(animation, vrm.humanoid, vrm.meta.metaVersion)
  const tracks: THREE.KeyframeTrack[] = [...rotation.values(), ...translation.values()]
  return new THREE.AnimationClip(name, animation.duration, tracks)
}
