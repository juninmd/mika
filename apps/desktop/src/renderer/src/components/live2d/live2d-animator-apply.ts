import { WIND_MAX, windInto } from '../avatar/vrm-wind'
import { hasModelParam } from './live2d-animator-parameter-probe'
import type { AnimatorState } from './live2d-animator-state'
import { getPostureForExpression } from './live2d-body-rig'
import type { Live2DModelInstance } from './live2d-loader'
import { lerp } from './live2d-physics'

export type Live2DCoreModel = Live2DModelInstance['internalModel']['coreModel']

function hasParam(s: AnimatorState, name: string): boolean {
  return hasModelParam(s.supportedParams, name)
}

function isYielded(s: AnimatorState, paramId: string): boolean {
  if (!s.activeAction) return false
  if (s.isSpeaking && (s.mouthOpenParams.includes(paramId) || paramId === s.mouthFormParam)) return false
  return s.activeAction.params.has(paramId)
}

function write(s: AnimatorState, cm: Live2DCoreModel, paramId: string, value: number): void {
  if (isYielded(s, paramId)) return
  cm.setParameterValueById(paramId, value)
}

function applyOptionalModelParams(s: AnimatorState, cm: Live2DCoreModel): void {
  if (hasParam(s, 'ParamBodyAngleY')) {
    write(s, cm, 'ParamBodyAngleY', s.bodyY.current + s.chest.current)
  }

  if (hasParam(s, 'ParamChest')) {
    write(s, cm, 'ParamChest', s.chest.current)
  }

  if (hasParam(s, 'ParamEyeBallForm')) {
    let eyeBallForm = 0
    if (s.currentExpression === 'evil') eyeBallForm = -0.8
    else if (s.currentExpression === 'surprised') eyeBallForm = -0.5
    write(s, cm, 'ParamEyeBallForm', eyeBallForm)
  }

  if (hasParam(s, 'ParamTear')) {
    cm.setParameterValueById('ParamTear', s.tear.current)
  }

  if (hasParam(s, 'FaceAngleX')) {
    cm.setParameterValueById('FaceAngleX', s.headX.current)
  }
  if (hasParam(s, 'FaceAngleY')) {
    cm.setParameterValueById('FaceAngleY', s.headY.current)
  }
  if (hasParam(s, 'EyeBallX')) {
    cm.setParameterValueById('EyeBallX', s.eyeX.current)
  }
  if (hasParam(s, 'EyeBallY')) {
    cm.setParameterValueById('EyeBallY', s.eyeY.current)
  }
}

function blushAndHandTargetsForExpression(s: AnimatorState): {
  targetBlush: number
  targetHandR: number
  targetHandL: number
} {
  if (['happy', 'surprised', 'excited'].includes(s.currentExpression)) {
    return { targetBlush: 0.5, targetHandR: 0, targetHandL: 0 }
  }
  if (s.currentExpression === 'embarrassed') {
    return { targetBlush: 1.0, targetHandR: 0, targetHandL: 0 }
  }
  if (s.currentExpression === 'love') {
    return { targetBlush: 1.0, targetHandR: 1, targetHandL: 0 }
  }
  if (s.currentExpression === 'evil') {
    return { targetBlush: 0.2, targetHandR: 0, targetHandL: 1 }
  }
  if (s.currentExpression === 'thinking') {
    return { targetBlush: 0, targetHandR: 1, targetHandL: 0 }
  }
  return { targetBlush: 0, targetHandR: 0, targetHandL: 0 }
}

/** Physics wind at full gust, in Cubism force units (gravity is 1): enough to sway, not to flip. */
export const PHYSICS_WIND_SCALE = 0.7

type PhysicsOptions = { wind: { x: number; y: number } }

/**
 * Cubism physics owns the hair/cloth params listed in the skin's physics3.json and rewrites them on
 * every `model.update`, after anything the rig wrote — so wind written as params never reaches the
 * mesh. It enters as a physics input instead: Cubism adds `wind` as force to each pendulum particle.
 */
export function applyPhysicsWind(s: AnimatorState, physics: { getOption?: () => PhysicsOptions } | undefined): void {
  const option = physics?.getOption?.()
  if (!option) return
  option.wind.x = (s.wind.x / WIND_MAX) * PHYSICS_WIND_SCALE
  option.wind.y = 0
}

export function applyToModel(s: AnimatorState, cm: Live2DCoreModel, dt: number) {
  windInto(s.wind, s.time, s.windStrength)
  write(s, cm, 'ParamAngleX', s.headX.current + s.proceduralOffsetX)
  write(s, cm, 'ParamAngleY', s.headY.current + s.proceduralOffsetY)
  write(s, cm, 'ParamAngleZ', s.headZ.current + s.proceduralOffsetZ)

  write(s, cm, 'ParamBodyAngleX', s.bodyX.current + s.hips.current * 0.5)
  write(s, cm, 'ParamBodyAngleZ', s.bodyZ.current + s.hips.current * 0.3)

  write(s, cm, 'ParamEyeBallX', s.eyeX.current)
  write(s, cm, 'ParamEyeBallY', s.eyeY.current)

  write(s, cm, 'ParamBrowLY', s.browY.current)
  write(s, cm, 'ParamBrowRY', s.browY.current)
  write(s, cm, 'ParamBrowLAngle', s.browAngle.current)
  write(s, cm, 'ParamBrowRAngle', s.browAngle.current)

  cm.setParameterValueById('ParamBreath', s.breathVal)

  applyOptionalModelParams(s, cm)

  write(s, cm, 'ParamEyeLOpen', s.blinkValue)
  write(s, cm, 'ParamEyeROpen', s.blinkValue)

  for (const param of s.mouthOpenParams) {
    write(s, cm, param, s.lipSync.value)
  }

  const { targetBlush, targetHandR, targetHandL } = blushAndHandTargetsForExpression(s)

  s.blushVal = lerp(s.blushVal, targetBlush, dt * 3.0)
  write(s, cm, 'ParamTere', s.blushVal)

  if (hasParam(s, 'ParamHandChangeR')) {
    cm.setParameterValueById('ParamHandChangeR', targetHandR)
  }
  if (hasParam(s, 'ParamHandChangeL')) {
    cm.setParameterValueById('ParamHandChangeL', targetHandL)
  }

  applyBodyRig(s, cm, dt)

  if (s.probeOverride) {
    for (const [id, value] of Object.entries(s.probeOverride)) cm.setParameterValueById(id, value)
  }
}

function currentPosture(s: AnimatorState) {
  const base = getPostureForExpression(s.currentExpression)
  if (s.nerdVal <= 0.01) return base
  return { ...base, lift: base.lift - s.nerdVal * 0.8, energy: base.energy * (1 - s.nerdVal * 0.25) }
}

function applyBodyRig(s: AnimatorState, cm: Live2DCoreModel, dt: number): void {
  const rig = s.bodyRig
  if (!rig) return

  rig.update(
    {
      dt,
      time: s.time,
      windSide: s.wind.x / WIND_MAX,
      windGust: Math.hypot(s.wind.x, s.wind.z) / WIND_MAX,
      bodySwayVelocity: s.bodyX.velocity,
      bodyHeaveVelocity: s.bodyY.velocity,
      headSwayVelocity: s.headX.velocity,
      breath: s.breathVal,
      posture: currentPosture(s),
      shiver: s.shiverVal,

      speechEnergy: s.isSpeaking ? s.lipSync.value : 0,
    },
    (paramId, value) => write(s, cm, paramId, value),
  )

  if (s.activeAction && s.actionDrivesBody) rig.relax(dt)
}

export function readModelParams(cm: Live2DCoreModel, ids: string[]): Record<string, number> {
  const out: Record<string, number> = {}
  for (const id of ids) {
    try {
      out[id] = cm.getParameterValueById?.(id) ?? Number.NaN
    } catch {
      out[id] = Number.NaN
    }
  }
  return out
}
