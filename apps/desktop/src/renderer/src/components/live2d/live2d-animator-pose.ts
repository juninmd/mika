import {
  BLINK_INTERVAL_MAX,
  BLINK_INTERVAL_MIN,
  IDLE_MOTION_MAX_INTERVAL,
  IDLE_MOTION_MIN_INTERVAL,
  type Mood,
  randomBetween,
} from './live2d-animator-config'
import type { AnimatorState } from './live2d-animator-state'

export function applyMoodSprings(s: AnimatorState, mood: Mood): void {
  s.currentMood = mood

  let stiffnessMult = 1.0
  let dampingMult = 1.0

  switch (mood) {
    case 'energetic':
      stiffnessMult = 1.5
      dampingMult = 0.8
      break
    case 'tired':
      stiffnessMult = 0.5
      dampingMult = 1.5
      break
    case 'focused':
      stiffnessMult = 1.2
      dampingMult = 1.2
      break
  }

  const springs = [s.headX, s.headY, s.headZ, s.bodyX, s.bodyZ]
  springs.forEach((spring) => {
    if (spring === s.headX || spring === s.headY) {
      spring.stiffness = 80 * stiffnessMult
      spring.damping = 6 * dampingMult
    } else if (spring === s.headZ) {
      spring.stiffness = 60 * stiffnessMult
      spring.damping = 8 * dampingMult
    } else {
      spring.stiffness = 40 * stiffnessMult
      spring.damping = 8 * dampingMult
    }
  })
}

export function resetPoseState(s: AnimatorState): void {
  const springs = [
    s.headX,
    s.headY,
    s.headZ,
    s.bodyX,
    s.bodyY,
    s.bodyZ,
    s.eyeX,
    s.eyeY,
    s.chest,
    s.hips,
    s.browY,
    s.browAngle,
    s.tear,
  ]
  for (const spring of springs) {
    spring.current = 0
    spring.target = 0
    spring.velocity = 0
  }

  applyMoodSprings(s, s.currentMood)

  s.proceduralOffsetX = 0
  s.proceduralOffsetY = 0
  s.proceduralOffsetZ = 0
  s.shiverVal = 0
  s.glitchVal = 0
  s.nerdVal = 0
  s.blushVal = 0
  s.breathVal = 0
  s.mouthOpenness = 0

  s.blinkValue = 1
  s.blinkState = 'open'
  s.isDoubleBlink = false

  s.nodTimer = 0
  s.stretchTimer = 0
  s.sighTimer = 0
  s.laughTimer = 0
  s.nerdTimer = 0
  s.yawnTimer = 0
  s.cringeTimer = 0
  s.dizzyTimer = 0

  s.activeAction = null
  s.actionDrivesBody = false
}

export function prepareFreeze(s: AnimatorState): void {
  s.deterministic = true
  s.noise.seed(0.42)
  s.hasFocus = false
  s.focusTarget = { x: 0, y: 0 }
  s.saccadeTarget = { x: 0, y: 0 }
  s.saccadeTimer = 0
  s.nextSaccadeTime = Number.POSITIVE_INFINITY
  s.idleTimer = 0
  s.nextIdleTime = Number.POSITIVE_INFINITY
  s.blinkTimer = 0
  s.nextBlinkTime = Number.POSITIVE_INFINITY
  s.time = 0
  s.running = true
  resetPoseState(s)
  s.bodyRig?.freeze()
}

export function releaseFreeze(s: AnimatorState): void {
  s.deterministic = false
  s.bodyRig?.unfreeze()
  s.nextSaccadeTime = randomBetween(0.5, 2.5)
  s.nextIdleTime = randomBetween(IDLE_MOTION_MIN_INTERVAL / 1000, IDLE_MOTION_MAX_INTERVAL / 1000)
  s.nextBlinkTime = randomBetween(BLINK_INTERVAL_MIN, BLINK_INTERVAL_MAX)
  s.running = true
}
