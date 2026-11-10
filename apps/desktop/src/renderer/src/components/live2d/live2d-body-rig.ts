import type { ParameterRange } from './live2d-animator-parameter-probe'
import {
  type BodyRigFrame,
  JOINT_CANDIDATES,
  JOINT_GAIN,
  JOINT_SPRING,
  type JointName,
  type ResolvedJoint,
} from './live2d-body-rig-tables'
import { clamp, lerp, PerlinNoise1D, Spring } from './live2d-physics'

export { type BodyPosture, type BodyRigFrame, getPostureForExpression, type JointName } from './live2d-body-rig-tables'

export class Live2DBodyRig {
  private readonly joints: ResolvedJoint[] = []
  private readonly byName = new Map<JointName, ResolvedJoint>()
  private readonly noise = new PerlinNoise1D()

  private weight = new Spring(0, 3.2, 3.4, 1)
  private weightTimer = 0
  private nextWeightShift = 0
  private deterministic = false

  constructor(ranges: ReadonlyMap<string, ParameterRange>) {
    for (const name of Object.keys(JOINT_CANDIDATES) as JointName[]) {
      const params = JOINT_CANDIDATES[name]
        .map((id) => ranges.get(id))
        .filter((range): range is ParameterRange => range !== undefined)

        .filter((range) => range.max > range.min)
      if (params.length === 0) continue

      const [stiffness, damping, mass] = JOINT_SPRING[name]
      const joint: ResolvedJoint = { name, params, spring: new Spring(0, stiffness, damping, mass) }
      this.joints.push(joint)
      this.byName.set(name, joint)
    }

    this.nextWeightShift = this.randomBetween(8, 18)
  }

  get isEmpty(): boolean {
    return this.joints.length === 0
  }

  listDrivenParams(): string[] {
    return this.joints.flatMap((joint) => joint.params.map((param) => param.id))
  }

  describe(): Record<string, string[]> {
    const out: Record<string, string[]> = {}
    for (const joint of this.joints) out[joint.name] = joint.params.map((param) => param.id)
    return out
  }

  private randomBetween(min: number, max: number): number {
    if (this.deterministic) return (min + max) / 2
    return Math.random() * (max - min) + min
  }

  private updateWeightShift(dt: number, energy: number): void {
    this.weightTimer += dt
    if (this.weightTimer >= this.nextWeightShift) {
      this.weightTimer = 0

      this.nextWeightShift = this.randomBetween(8, 18) / clamp(energy, 0.4, 2)
      const side = this.weight.target >= 0 ? -1 : 1
      this.weight.target = side * this.randomBetween(0.45, 1)
    }
    this.weight.update(dt)
  }

  update(frame: BodyRigFrame, write: (paramId: string, value: number) => void): void {
    if (this.joints.length === 0) return

    const { dt, time, posture } = frame
    this.updateWeightShift(dt, posture.energy)

    const weight = this.weight.current
    const heave = (frame.breath - 0.5) * 2

    const swayDrag = clamp(-frame.bodySwayVelocity * 0.02, -1, 1)
    const heaveDrag = clamp(-frame.bodyHeaveVelocity * 0.015, -1, 1)

    const driftL = this.noise.get(time * 0.24) * 0.5
    const driftR = this.noise.get(time * 0.21 + 57) * 0.5

    const energy = posture.energy
    const jitter = frame.shiver > 0.02 ? Math.sin(time * 34) * frame.shiver * 0.35 : 0

    const speechBeat = frame.speechEnergy > 0.05 ? Math.sin(time * 6.3) * frame.speechEnergy * 0.3 : 0

    const upperL = (swayDrag + heaveDrag * 0.6 + driftL - weight * 0.55 + posture.lift + heave * 0.12) * energy
    const upperR = (swayDrag + heaveDrag * 0.6 + driftR + weight * 0.55 + posture.lift + heave * 0.12) * energy

    this.driveChain('upperArmL', 'foreArmL', 'handL', upperL, jitter + speechBeat, dt)
    this.driveChain('upperArmR', 'foreArmR', 'handR', upperR, -jitter + speechBeat, dt)

    this.driveJoint('torso', (heave * 0.5 + posture.lift * 0.6 + weight * 0.3 + swayDrag * 0.4) * energy, dt)

    const shoulderDrive = (heave * 0.25 + posture.shoulderTension + Math.abs(weight) * 0.15) * energy
    this.driveJoint('shoulder', shoulderDrive, dt)

    this.driveJoint('shoulderL', shoulderDrive - weight * 0.45 * energy, dt)
    this.driveJoint('shoulderR', shoulderDrive + weight * 0.45 * energy, dt)

    const headDrag = clamp(-frame.headSwayVelocity * 0.035, -1, 1)
    this.driveJoint('ahoge', clamp(headDrag * 1.6 + heaveDrag * 0.8 + jitter * 2 + driftL * 0.5, -1, 1), dt)
    this.driveHair(frame, headDrag, dt)

    this.driveJoint('legL', -weight * 0.8 * energy, dt)
    this.driveJoint('legR', weight * 0.8 * energy, dt)
    this.driveJoint('knee', (Math.abs(weight) * 0.7 - 0.2 + heave * 0.1) * energy, dt)

    for (const joint of this.joints) this.writeJoint(joint, write)
  }

  /** Wind through the hair (parity with the VRM spring bones), on top of the head's own drag. */
  private driveHair(frame: BodyRigFrame, headDrag: number, dt: number): void {
    const flutter = Math.sin(frame.time * 3.1) * frame.windGust * 0.25
    this.driveJoint('hairSide', frame.windSide * 1.4 + headDrag * 0.6 + flutter, dt)
    this.driveJoint('hairFront', frame.windSide * 0.8 + headDrag * 0.4 + flutter * 0.5, dt)
    this.driveJoint('hairBack', frame.windSide * 0.9 + frame.windGust * 0.4 + headDrag * 0.3, dt)
  }

  private driveChain(upper: JointName, fore: JointName, hand: JointName, drive: number, tip: number, dt: number): void {
    const upperJoint = this.driveJoint(upper, drive, dt)
    const foreDrive = (upperJoint?.spring.current ?? drive) * 1.15
    const foreJoint = this.driveJoint(fore, foreDrive, dt)
    const handDrive = (foreJoint?.spring.current ?? foreDrive) * 1.25 + tip
    this.driveJoint(hand, handDrive, dt)
  }

  private driveJoint(name: JointName, target: number, dt: number): ResolvedJoint | undefined {
    const joint = this.byName.get(name)
    if (!joint) return undefined
    joint.spring.target = clamp(target, -1, 1)
    joint.spring.update(dt)
    return joint
  }

  private writeJoint(joint: ResolvedJoint, write: (paramId: string, value: number) => void): void {
    const norm = clamp(joint.spring.current, -1, 1) * JOINT_GAIN[joint.name]
    for (const param of joint.params) {
      const span = norm >= 0 ? param.max - param.rest : param.rest - param.min
      write(param.id, clamp(param.rest + norm * span, param.min, param.max))
    }
  }

  relax(dt: number): void {
    for (const joint of this.joints) {
      joint.spring.target = 0
      joint.spring.current = lerp(joint.spring.current, 0, clamp(dt * 3, 0, 1))
      joint.spring.velocity = lerp(joint.spring.velocity, 0, clamp(dt * 3, 0, 1))
    }
  }

  freeze(): void {
    this.deterministic = true
    this.noise.seed(0.42)
    this.weightTimer = 0
    this.nextWeightShift = Number.POSITIVE_INFINITY
    this.weight.current = 0
    this.weight.target = 0
    this.weight.velocity = 0
    for (const joint of this.joints) {
      joint.spring.current = 0
      joint.spring.target = 0
      joint.spring.velocity = 0
    }
  }

  unfreeze(): void {
    this.deterministic = false
    this.weightTimer = 0
    this.nextWeightShift = this.randomBetween(8, 18)
  }
}
