import { IDLE_MOTION_MAX_INTERVAL, IDLE_MOTION_MIN_INTERVAL, randomBetween } from './live2d-animator-config'
import {
  cringePose,
  dizzyPose,
  gaspPose,
  jumpPose,
  laughPose,
  type ReactFn,
  shiverPose,
  sighPose,
  stretchPose,
  tiltPose,
} from './live2d-animator-gestures'
import type { AnimatorState } from './live2d-animator-state'

export interface IdleHooks {
  react: ReactFn
  shrug: () => void
  playAnyAction: () => void
}

function idleShrugOrStretch(s: AnimatorState, hooks: IdleHooks): void {
  s.bodyZ.target = 5
  s.headY.target = -5
  s.headZ.target = 5
  if (Math.random() > 0.5) hooks.shrug()
  else stretchPose(s)
}

function idleEyeFlickAndTilt(s: AnimatorState): void {
  s.eyeX.velocity = Math.random() > 0.5 ? 50 : -50
  tiltPose(s, Math.random() > 0.5 ? 'left' : 'right')
}

function idleTilt(s: AnimatorState): void {
  tiltPose(s, Math.random() > 0.5 ? 'left' : 'right')
}

function idleSighOrLaugh(s: AnimatorState, hooks: IdleHooks): void {
  if (Math.random() > 0.5) sighPose(s)
  else laughPose(s, hooks.react)
}

function idleEvilLaugh(s: AnimatorState, hooks: IdleHooks): void {
  hooks.react('evil')
  s.glitchVal = 0.3
  s.shiverVal = 0.4
  s.eyeX.velocity = 20
  s.laughTimer = 0.5
}

function idleNerdMoment(s: AnimatorState): void {
  s.nerdVal = 0.8
  s.shiverVal = 0.2
  s.nerdTimer = 2.0
}

function idleEyeDart(s: AnimatorState): void {
  s.eyeX.velocity = Math.random() > 0.5 ? 100 : -100
  s.headX.velocity = Math.random() > 0.5 ? 20 : -20
  s.delayedIdleMotionTimer = setTimeout(() => {
    s.delayedIdleMotionTimer = null
    s.eyeX.velocity = Math.random() > 0.5 ? 80 : -80
  }, 600)
}

function playIdleYawn(s: AnimatorState): void {
  if (s.currentMood !== 'tired' && s.currentMood !== 'neutral') return
  s.bodyZ.target = -5
  s.headY.velocity = -10
  if (s.blinkState === 'open') {
    s.blinkState = 'closing'
  }
  s.yawnTimer = 1.5
}

function idleMotionTable(s: AnimatorState, hooks: IdleHooks): Array<{ upTo: number; play: () => void }> {
  return [
    { upTo: 0.1, play: () => idleShrugOrStretch(s, hooks) },
    { upTo: 0.2, play: () => idleEyeFlickAndTilt(s) },
    { upTo: 0.3, play: () => idleTilt(s) },
    { upTo: 0.4, play: () => idleSighOrLaugh(s, hooks) },
    { upTo: 0.5, play: () => hooks.react('thinking') },
    { upTo: 0.55, play: () => hooks.react('smug') },
    { upTo: 0.6, play: () => idleEvilLaugh(s, hooks) },
    { upTo: 0.7, play: () => idleNerdMoment(s) },
    { upTo: 0.8, play: () => idleEyeDart(s) },
    { upTo: 0.9, play: () => playIdleYawn(s) },
    { upTo: 0.92, play: () => cringePose(s, hooks.react) },
    { upTo: 0.93, play: () => gaspPose(s, hooks.react) },
    { upTo: 0.94, play: () => dizzyPose(s, hooks.react) },
    { upTo: 0.96, play: () => jumpPose(s, hooks.react) },
    { upTo: 0.98, play: () => shiverPose(s) },
    { upTo: 1, play: () => hooks.playAnyAction() },
  ]
}

function playRandomIdleMotion(s: AnimatorState, hooks: IdleHooks, rand: number): void {
  for (const entry of idleMotionTable(s, hooks)) {
    if (rand < entry.upTo) {
      entry.play()
      return
    }
  }
}

export function updateIdleMotions(s: AnimatorState, hooks: IdleHooks, dt: number) {
  if (s.isSpeaking || s.currentExpression === 'sleep') return

  s.idleTimer += dt
  if (s.idleTimer > s.nextIdleTime) {
    s.idleTimer = 0
    s.nextIdleTime = randomBetween(IDLE_MOTION_MIN_INTERVAL / 1000, IDLE_MOTION_MAX_INTERVAL / 1000)
    playRandomIdleMotion(s, hooks, Math.random())
  }
}
