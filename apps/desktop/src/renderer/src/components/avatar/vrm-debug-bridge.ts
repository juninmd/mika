import type { ArmPose } from './vrm-body-rig'
import type { VowelWeightsLike } from './vrm-mouth'

// Same debug bridge Live2D has, for VRM: play an action and read bone rotations to answer "did it move the arm?".
export interface VrmDebugBridge {
  playAction(name: string): void

  /** Current arm rotations, including gesture depth axes. */
  readArmRotations(): ArmPose

  /** Life systems state: gaze, blink, mouth, springs, post-fx and GPU memory. */
  readLife(): VrmLifeSnapshot

  /** Holds one long idle state (`vrm-idle-states.ts`) now; false for an unknown name. */
  playIdleState(name: string): boolean

  /** Freezes the autonomous idle pose scheduler — E2E needs a stable rest pose to measure gestures against. */
  setIdleEnabled(enabled: boolean): void

  /** Feeds synthetic volume/vowels into the real lip-sync pipeline — TTS audio is off in E2E. */
  driveLipSync(volume: number, vowels: VowelWeightsLike): void
}

export interface VrmLifeSnapshot {
  headYaw: number
  headPitch: number
  neckYaw: number
  blink: number
  happy: number
  mouth: number
  postFx: boolean
  /** Sum of spring bone rotations: changes when breeze or inertia moves them. */
  springPose: number
  springFinite: boolean
  textures: number
  geometries: number
  canvasWidth: number
}

declare global {
  interface Window {
    __mikaVrm?: VrmDebugBridge
  }
}

export function installVrmDebugBridge(bridge: VrmDebugBridge): () => void {
  window.__mikaVrm = bridge
  return () => {
    if (window.__mikaVrm === bridge) window.__mikaVrm = undefined
  }
}
