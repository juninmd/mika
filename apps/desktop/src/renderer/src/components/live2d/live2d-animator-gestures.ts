import type { AnimatorState } from './live2d-animator-state'

export type ReactFn = (expression: string) => void

export const GESTURE_ACTIONS: Record<string, string[]> = {
  greet: ['wave', 'happy', 'nod'],
  shrug: ['shrug', 'shakehead', 'thinking'],
  agree: ['nod', 'happy'],
}

export function nodPose(s: AnimatorState) {
  s.nodTimer = 0.4
}

export function jumpPose(s: AnimatorState, react: ReactFn) {
  s.bodyZ.velocity = 15
  s.headY.velocity = 20
  react('surprised')
}

export function shiverPose(s: AnimatorState) {
  s.shiverVal = 1.0
}

export function tiltPose(s: AnimatorState, direction: 'left' | 'right') {
  s.headZ.velocity = direction === 'left' ? -20 : 20
}

export function stretchPose(s: AnimatorState) {
  s.bodyY.velocity = 20
  s.chest.velocity = 30
  s.headY.velocity = 15
  s.headZ.velocity = 10

  if (s.blinkState === 'open') {
    s.blinkState = 'closing'
  }

  s.stretchTimer = 0.6
}

export function sighPose(s: AnimatorState) {
  s.chest.velocity = 40
  s.headY.velocity = 10

  s.sighTimer = 0.8
}

export function laughPose(s: AnimatorState, react: ReactFn) {
  react('laugh')
  s.bodyY.velocity = 25
  s.chest.velocity = 20
  s.headY.velocity = 15
  s.mouthOpenness = 0.8

  s.laughTimer = 0.8
}

export function headPatPose(s: AnimatorState, react: ReactFn) {
  s.headY.target = -15
  s.bodyZ.target = -5
  s.headZ.velocity = 10
  s.eyeY.target = -0.5
  react('love')
  if (s.blinkState === 'open') {
    s.blinkState = 'closing'
  }
}

export function cringePose(s: AnimatorState, react: ReactFn) {
  s.cringeTimer = 0.6
  s.bodyY.velocity = -20
  s.headY.velocity = -10
  react('shy')
}

export function gaspPose(s: AnimatorState, react: ReactFn) {
  s.headY.velocity = 30
  s.chest.velocity = 50
  react('surprised')
}

export function dizzyPose(s: AnimatorState, react: ReactFn) {
  s.dizzyTimer = 1.0
  react('confused')
}
