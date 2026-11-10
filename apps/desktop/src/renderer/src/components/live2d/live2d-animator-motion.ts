import {
  BREATH_FREQ_PRIMARY,
  BREATH_FREQ_SECONDARY,
  DOUBLE_BLINK_CHANCE,
  randomBetween,
} from './live2d-animator-config'
import type { AnimatorState } from './live2d-animator-state'
import { clamp, lerp } from './live2d-physics'

export function updateFocus(s: AnimatorState, _dt: number) {
  if (s.currentExpression === 'sleep') {
    s.headX.target = 0
    s.headY.target = -8
    s.headZ.target = 0
    s.bodyX.target = 0
    s.bodyZ.target = 0
    return
  }

  let tx = 0,
    ty = 0

  if (s.hasFocus) {
    tx = s.focusTarget.x
    ty = s.focusTarget.y
  } else {
    tx = s.noise.get(s.time * 0.2) * 0.4
    ty = s.noise.get(s.time * 0.15 + 100) * 0.2
  }

  s.headX.target = tx * 30
  s.headY.target = ty * 30

  s.headZ.target = tx * -5

  s.bodyX.target = tx * 20
  s.bodyZ.target = tx * 10

  if (s.hasFocus) {
    s.eyeX.target = tx + s.saccadeTarget.x * 0.2
    s.eyeY.target = ty + s.saccadeTarget.y * 0.2
  } else {
    s.eyeX.target = tx + s.saccadeTarget.x
    s.eyeY.target = ty + s.saccadeTarget.y
  }
}

export function updateSaccades(s: AnimatorState, dt: number) {
  s.saccadeTimer += dt

  let saccadeMultiplier = 1.0
  if (s.currentExpression === 'panic') saccadeMultiplier = 0.3
  else if (s.currentExpression === 'bored' || s.currentExpression === 'sleep') saccadeMultiplier = 3.0

  if (s.saccadeTimer > s.nextSaccadeTime * saccadeMultiplier) {
    s.saccadeTimer = 0

    s.nextSaccadeTime = randomBetween(0.5, 2.5)

    s.saccadeTarget.x = randomBetween(-0.3, 0.3)
    s.saccadeTarget.y = randomBetween(-0.2, 0.2)

    if (Math.random() < 0.3 && s.blinkState === 'open') {
      s.blinkState = 'closing'
      s.blinkTimer = 0
      s.isDoubleBlink = Math.random() < DOUBLE_BLINK_CHANCE
    }
  }
}

export function updatePhysics(s: AnimatorState, dt: number) {
  const heave = (s.breathVal - 0.5) * 2.0

  s.chest.target = heave * 5

  s.hips.target = heave * 2

  let gx = 0,
    gy = 0
  if (s.glitchVal > 0.05) {
    if (Math.random() < s.glitchVal * 0.5) {
      gx = (Math.random() - 0.5) * 10 * s.glitchVal
      gy = (Math.random() - 0.5) * 10 * s.glitchVal
    }
  }

  const finalHeadY = s.headY.target + heave * 1.5 + gy
  const finalBodyX = s.bodyX.target + Math.sin(s.time * 10) * s.shiverVal * 2 + gx

  const finalBodyY = s.bodyY.target + s.headY.velocity * -0.05
  const finalBodyZ = s.bodyZ.target + s.headX.velocity * -0.05

  const originalHeadY = s.headY.target
  const originalBodyX = s.bodyX.target
  const originalBodyY = s.bodyY.target
  const originalBodyZ = s.bodyZ.target

  s.headY.target = finalHeadY
  s.bodyX.target = finalBodyX
  s.bodyY.target = finalBodyY
  s.bodyZ.target = finalBodyZ

  s.headX.update(dt)
  s.headY.update(dt)
  s.headZ.update(dt)
  s.bodyX.update(dt)
  s.bodyY.update(dt)
  s.bodyZ.update(dt)
  s.eyeX.update(dt)
  s.eyeY.update(dt)
  s.browY.update(dt)
  s.browAngle.update(dt)
  s.chest.update(dt)
  s.hips.update(dt)

  s.headY.target = originalHeadY
  s.bodyX.target = originalBodyX
  s.bodyY.target = originalBodyY
  s.bodyZ.target = originalBodyZ

  s.tear.target =
    s.currentExpression === 'sad' || s.currentExpression === 'scared' || s.currentExpression === 'panic' ? 1.0 : 0.0
  s.tear.update(dt)

  s.proceduralOffsetX = lerp(s.proceduralOffsetX, 0, dt * 5)
  s.proceduralOffsetY = lerp(s.proceduralOffsetY, 0, dt * 5)
  s.shiverVal = lerp(s.shiverVal, 0, dt * 2)
  s.glitchVal = lerp(s.glitchVal, 0, dt * 1.5)
}

export function updateBreathing(s: AnimatorState, _dt: number) {
  let f1 = BREATH_FREQ_PRIMARY
  let breathDepth = 1.0

  if (s.currentExpression === 'sleep') {
    f1 = 0.15
    breathDepth = 0.5
  } else if (s.currentExpression === 'bored') {
    f1 = 0.2
    breathDepth = 0.8
  } else if (s.currentExpression === 'excited' || s.currentExpression === 'laugh') {
    f1 = 0.6
    breathDepth = 1.2
  } else if (s.currentExpression === 'panic') {
    f1 = 0.8
    breathDepth = 1.5
  } else if (s.currentExpression === 'tired') {
    f1 = 0.25
    breathDepth = 1.3
  }

  const t = s.time

  const base = (Math.sin(t * Math.PI * 2 * f1) + 1) * 0.5
  const var_ = Math.sin(t * Math.PI * 2 * BREATH_FREQ_SECONDARY) * 0.1
  s.breathVal = clamp((base + var_) * breathDepth, 0, 1)
}
