/**
 * Always-on ambient motion, added on top of every pose: sway, head tilt, breathing and arm swing.
 * Incommensurate frequencies keep it from ever settling into a loop the eye can memorise.
 */
import type { VRM } from '@pixiv/three-vrm'

export interface IdleMotion {
  spineZ: number
  spineY: number
  chestX: number
  headX: number
  headY: number
  headZ: number
  hipsY: number
  armSwing: number
  // Whole-body turn (rad) and weight shift / breathing bob (m), on the model root.
  bodyTurn: number
  bodyShift: number
  bodyBob: number
}

const wave = (t: number, amplitude: number, rate: number, phase = 0) => amplitude * Math.sin(t * rate + phase)

export function idleMotion(t: number): IdleMotion {
  return {
    spineZ: wave(t, 0.045, 0.83, 1) + wave(t, 0.02, 0.29),
    spineY: wave(t, 0.04, 0.52, 0.4) + wave(t, 0.015, 1.37),
    chestX: wave(t, 0.028, 1.6) + wave(t, 0.008, 3.2, 0.5),
    headX: wave(t, 0.035, 0.61, 0.5) + wave(t, 0.015, 1.7),
    headY: wave(t, 0.09, 0.43) + wave(t, 0.04, 1.17, 1),
    headZ: wave(t, 0.08, 0.71, 2) + wave(t, 0.035, 1.93, 0.3),
    hipsY: wave(t, 0.025, 0.39, 2),
    armSwing: wave(t, 0.04, 0.83, -0.5),
    bodyTurn: wave(t, 0.07, 0.31, 0.7) + wave(t, 0.025, 0.77),
    bodyShift: wave(t, 0.012, 0.28, 1.4),
    bodyBob: wave(t, 0.004, 1.6, 0.4),
  }
}

type Humanoid = VRM['humanoid']

/** Adds the motion over whatever pose the frame already wrote; `weight` fades it under actions. */
export function applyIdleMotion(humanoid: Humanoid, m: IdleMotion, weight: number): void {
  const add = (bone: Parameters<Humanoid['getNormalizedBoneNode']>[0], x: number, y: number, z: number) => {
    const node = humanoid.getNormalizedBoneNode(bone)
    if (!node) return
    node.rotation.x += x * weight
    node.rotation.y += y * weight
    node.rotation.z += z * weight
  }
  add('spine', 0, m.spineY, m.spineZ)
  add('chest', m.chestX, 0, 0)
  add('head', m.headX, m.headY, m.headZ)
  add('hips', 0, m.hipsY, 0)
  // Arms swing opposite the torso's lean, the way a relaxed arm hangs back from a sway.
  add('leftUpperArm', m.armSwing * 0.5, 0, -m.armSwing)
  add('rightUpperArm', m.armSwing * 0.5, 0, -m.armSwing)
}
