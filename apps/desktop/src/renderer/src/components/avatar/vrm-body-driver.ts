import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { type SpringState, springTo } from './damped-spring'
import type { VrmActionClock } from './vrm-action-clock'
import { ARM_REST, type ArmPose, VrmArmDriver } from './vrm-body-rig'
import { VrmClipLayer } from './vrm-clip-layer'
import { IDLE_CLIP, VRM_ACTION_CLIPS, VRM_CLIP_URLS } from './vrm-clips'
import { type FidgetOffsets, VrmFidgetLayer } from './vrm-fidget'
import { GazeDriver, HEAD_BACK_LIMIT } from './vrm-gaze'
import { type IdleGate, VrmIdleScheduler } from './vrm-idle'
import { applyIdleMotion, idleMotion } from './vrm-idle-motion'
import { type IdleState, VrmIdleStateLayer } from './vrm-idle-states'
import {
  addBodyPoseOffsets,
  animateVrmHead,
  applyGaze,
  applyIdleArmSway,
  applyIdleSway,
  applySpeechGesture,
  readArmPose,
  writeArmPose,
} from './vrm-pose'
import { attachEyeTarget } from './vrm-scene-setup'

const POSE_KEYS = Object.keys(ARM_REST) as (keyof ArmPose)[]

export class VrmBodyDriver {
  readonly lookTarget = new THREE.Object3D()
  readonly eyeTarget = new THREE.Object3D()
  private readonly headOrigin = new THREE.Vector3()
  private readonly gaze = new GazeDriver()
  private readonly arms = new VrmArmDriver()
  private readonly idle = new VrmIdleScheduler()
  private readonly clips = new VrmClipLayer()
  private readonly fidgets = new VrmFidgetLayer()
  private readonly states = new VrmIdleStateLayer()
  /** Rest (or the action) plus the idle state's offsets; written in place every frame. */
  private readonly statePose: ArmPose = { ...ARM_REST }
  /** Reused gates: no object literal per frame. */
  private readonly quietGate: IdleGate = { busy: false, enabled: true }
  private readonly poseGate: IdleGate = { busy: false, enabled: true }
  private hour = new Date().getHours()
  private hourAt = 0
  private bodyPose: Readonly<ArmPose> = ARM_REST
  private nodImpulse = 0
  private jumpImpulse = 0
  private cursorX = 0
  private cursorY = 0
  private sceneBaseY = 0
  private sceneBaseX = 0
  private sceneBaseTurn = 0
  private readonly ambient: SpringState = { value: 1, velocity: 0 }

  constructor(scene: THREE.Scene) {
    scene.add(this.lookTarget, this.eyeTarget)
    this.lookTarget.position.set(0, 1.4, 2)
  }

  attach(vrm: VRM): void {
    attachEyeTarget(vrm, this.eyeTarget)
    this.sceneBaseY = vrm.scene.position.y
    this.sceneBaseX = vrm.scene.position.x
    this.sceneBaseTurn = vrm.scene.rotation.y
    // VRM loads in T-pose; apply the same ARM_REST actions return to, or the first action "corrects" the pose.
    this.applyArmPose(vrm, ARM_REST)
  }

  /** Loads the captured clips; the procedural rig keeps her moving if they fail. */
  loadClips(vrm: VRM): Promise<void> {
    return this.clips.attach(vrm, VRM_CLIP_URLS)
  }

  play(name: string): void {
    const clip = VRM_ACTION_CLIPS[name as keyof typeof VRM_ACTION_CLIPS] ?? name
    if (this.clips.play(clip, { fade: 0.35 })) return
    if (name === 'nod') this.nod()
    this.arms.play(name)
  }

  /** Velocidade angular da cabeça (rad/s) — a piscada acompanha giros largos. */
  get gazeSpeed(): number {
    return this.gaze.speed
  }

  playIdleState(state: IdleState): void {
    this.states.play(state)
  }

  /** Skin stage: idle states come within seconds, so a visitor sees them. */
  setShowcase(showcase: boolean): void {
    this.states.setShowcase(showcase)
  }

  nod(): void {
    this.nodImpulse = 1
  }

  jump(): void {
    this.jumpImpulse = 1
  }

  setCursorTarget(x: number, y: number): void {
    this.lookTarget.position.set(x * 1.2, 1.4 - y * 0.6, 2)
    this.cursorX = x
    this.cursorY = y
  }

  readArmRotations(vrm: VRM | null): ArmPose {
    return readArmPose(vrm?.humanoid, this.bodyPose)
  }

  update(
    vrm: VRM,
    dt: number,
    elapsed: number,
    action: VrmActionClock,
    speaking = false,
    volume = 0,
    idleEnabled = true,
  ): void {
    const humanoid = vrm.humanoid
    if (!humanoid) return
    this.clips.resetRoot()
    // Respiração orgânica multi-harmônica com suspiro sutil periódico a cada ~24s
    const deepBreath = Math.sin(elapsed * 0.26) ** 6 * 0.35
    const breath = Math.sin(elapsed * 1.05) + Math.sin(elapsed * 2.1) * 0.12 + deepBreath
    // Deslocamento de peso natural bi-harmônico (transferência orgânica de centro de gravidade)
    const weight = Math.sin(elapsed * 0.28) * 0.75 + Math.sin(elapsed * 0.11) * 0.25
    const weightLag = Math.sin(elapsed * 0.28 - 0.8) * 0.75 + Math.sin(elapsed * 0.11 - 0.8) * 0.25
    applyIdleSway(humanoid, breath, weight)
    const head = humanoid.getNormalizedBoneNode('head')
    if (head) {
      this.nodImpulse = Math.max(0, this.nodImpulse - dt * 1.5)
      animateVrmHead(head, this.nodImpulse, breath, weightLag, action.remaining, action.duration, action.name)
    }
    this.jumpImpulse = Math.max(0, this.jumpImpulse - dt * 2)
    const actionPose = this.arms.advance(dt, this.cursorX, this.cursorY) ?? ARM_REST
    const gesturing = this.arms.playing || (this.clips.playing ?? IDLE_CLIP) !== IDLE_CLIP
    this.quietGate.busy = gesturing || speaking
    this.quietGate.enabled = idleEnabled
    this.applyArmPose(vrm, this.withIdleState(actionPose, dt, elapsed))
    vrm.scene.position.y = this.sceneBaseY + Math.sin(this.jumpImpulse * Math.PI) * 0.08 + this.bodyPose.hipsY
    if (!this.arms.playing) {
      applyIdleArmSway(vrm.humanoid, breath, weightLag)
    }
    // A held idle state is the idle pose for now: no random full-pose action on top of it.
    this.poseGate.busy = this.quietGate.busy || this.states.current !== null
    this.poseGate.enabled = idleEnabled
    const idleAction = this.idle.tick(dt, this.poseGate)
    if (idleAction) this.play(idleAction)
    applySpeechGesture(vrm.humanoid, speaking, volume, elapsed)
    addBodyPoseOffsets(vrm.humanoid, this.bodyPose)
    this.driveIdleClip(idleEnabled)
    this.clips.blend(dt)
    // Never frozen: ambient sway stays on under poses and speech, only softened so gestures still read.
    // A critically damped spring, not a lerp: the crossfade into a gesture starts without a kink.
    const sway = springTo(this.ambient, this.arms.playing ? 0.35 : speaking ? 0.6 : 1, 4, dt)
    const ambient = idleMotion(elapsed)
    applyIdleMotion(humanoid, ambient, sway)
    // Micro-idles ride on top of the clip and the ambient sway, and yield to gestures and speech.
    const fidget = this.fidgets.advance(dt, this.quietGate, this.cursorX, this.cursorY)
    applyFidget(humanoid, fidget)
    // The lower body can't sway without the skirt clipping, so the whole model turns and shifts instead.
    vrm.scene.rotation.y = this.sceneBaseTurn + ambient.bodyTurn * sway
    vrm.scene.position.x = this.sceneBaseX + ambient.bodyShift * sway + fidget.bodyShift
    vrm.scene.position.y += ambient.bodyBob * sway
    // Nod and headshake actions own the bone while playing; gaze yields.
    const busy = this.nodImpulse > 0 || action.isPlaying('shakehead')
    applyGaze(vrm, this.gaze, this.lookTarget, this.eyeTarget, this.headOrigin, busy, dt)
    if (head) head.rotation.x = Math.max(head.rotation.x, -HEAD_BACK_LIMIT)
  }

  /** Adds the idle state (blended by its own spring) over the rest or action pose. */
  private withIdleState(pose: Readonly<ArmPose>, dt: number, elapsed: number): Readonly<ArmPose> {
    if (elapsed - this.hourAt > 60) {
      this.hour = new Date().getHours()
      this.hourAt = elapsed
    }
    const offsets = this.states.advance(dt, this.quietGate, this.hour)
    if (!this.states.current) return pose
    const out = this.statePose
    for (const key of POSE_KEYS) out[key] = pose[key] + offsets[key]
    return out
  }

  /**
   * The captured idle loop is the resting body; procedural gestures and held idle states take the
   * skeleton while they play (the clip would overwrite the arms they pose). It is autonomous motion
   * like the idle poses, so a backgrounded or reduced-motion window (`idleEnabled` off) holds the rest
   * pose instead of looping it.
   */
  private driveIdleClip(idleEnabled: boolean): void {
    const playing = this.clips.playing
    if (this.arms.playing || this.states.current || !idleEnabled) {
      if (playing === IDLE_CLIP) this.clips.stop(0.3)
    } else if (!playing) {
      this.clips.play(IDLE_CLIP, { loop: true, fade: 0.8 })
    }
  }

  private applyArmPose(vrm: VRM, pose: Readonly<ArmPose>): void {
    this.bodyPose = pose
    writeArmPose(vrm, pose)
  }
}

function applyFidget(humanoid: VRM['humanoid'], o: FidgetOffsets): void {
  const head = humanoid.getNormalizedBoneNode('head')
  if (head) {
    head.rotation.x += o.headX
    head.rotation.y += o.headY
    head.rotation.z += o.headZ
  }
  const spine = humanoid.getNormalizedBoneNode('spine')
  if (spine) spine.rotation.z += o.spineZ
  const chest = humanoid.getNormalizedBoneNode('chest')
  if (chest) chest.rotation.x += o.chestX
  const leftHand = humanoid.getNormalizedBoneNode('leftHand')
  if (leftHand) {
    leftHand.rotation.x += o.leftHandX
    leftHand.rotation.z += o.leftHandZ
  }
  const rightHand = humanoid.getNormalizedBoneNode('rightHand')
  if (rightHand) {
    rightHand.rotation.x += o.rightHandX
    rightHand.rotation.z += o.rightHandZ
  }
}
