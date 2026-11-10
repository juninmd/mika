import { sighPose } from './live2d-animator-gestures'
import type { AnimatorState } from './live2d-animator-state'

export function applyReactionPose(s: AnimatorState, expression: string, intensity: number): void {
  s.eyeX.stiffness = 150 * intensity
  s.eyeY.stiffness = 150 * intensity
  s.browY.target = 0
  s.browAngle.target = 0

  switch (expression) {
    case 'surprised':
      s.headY.velocity = 30 * intensity
      s.eyeX.stiffness = 200 * intensity
      s.browY.target = 0.8 * intensity
      s.browAngle.target = 0.5 * intensity
      s.shiverVal = 0.8 * intensity
      break
    case 'sad':
      s.headY.target = -10 * intensity
      s.eyeX.stiffness = 60 / intensity
      s.browY.target = -0.5 * intensity
      s.browAngle.target = -0.3 * intensity
      break
    case 'happy':
      s.headY.velocity = 15 * intensity
      s.bodyZ.velocity = 15 * intensity
      s.eyeX.stiffness = 150 * intensity
      s.browY.target = 0.3 * intensity
      break
    case 'angry':
      s.browY.target = -0.6 * intensity
      s.browAngle.target = 0.7 * intensity
      s.bodyZ.target = -5 * intensity
      s.headZ.velocity = 10 * intensity
      break
    case 'thinking':
      s.browY.target = 0.2 * intensity
      s.browAngle.target = -0.2 * intensity
      s.headZ.target = -10 * intensity
      s.eyeY.target = -0.5 * intensity
      break
    case 'wink':
      s.headZ.velocity = 10 * intensity
      s.eyeX.stiffness = 200 * intensity
      s.browY.target = 0.3 * intensity
      break
    case 'smug':
      s.headY.target = 5 * intensity
      s.headZ.target = 10 * intensity
      s.browY.target = 0.4 * intensity
      s.browAngle.target = -0.3 * intensity
      break
    case 'love':
      s.blushVal = 1.0 * intensity
      s.headY.velocity = 10 * intensity
      s.bodyZ.target = 5 * intensity
      s.browY.target = 0.2 * intensity
      break
    case 'evil':
      s.blushVal = 0.3 * intensity
      s.headY.target = -5 * intensity
      s.headZ.target = 5 * intensity
      s.browY.target = -0.8 * intensity
      s.browAngle.target = 1.0 * intensity
      s.shiverVal = 0.5 * intensity
      s.glitchVal = 0.4 * intensity
      break
    case 'panic':
      s.shiverVal = 1.0 * intensity
      s.eyeX.stiffness = 250 * intensity
      s.eyeY.stiffness = 250 * intensity
      s.browY.target = 0.8 * intensity
      s.browAngle.target = 0.5 * intensity
      s.bodyZ.target = -10 * intensity
      break
    case 'bored':
      s.headY.target = -5 * intensity
      s.eyeY.target = -0.5 * intensity
      s.browY.target = -0.2 * intensity
      s.browAngle.target = 0.1 * intensity
      s.eyeX.stiffness = 50 / intensity
      break
    case 'laugh':
      s.headY.velocity = 15 * intensity
      s.bodyY.velocity = 25 * intensity
      s.eyeX.stiffness = 150 * intensity
      s.browY.target = 0.4 * intensity
      break
    case 'disgusted':
      s.headY.target = -10 * intensity
      s.headZ.target = -5 * intensity
      s.browY.target = -0.5 * intensity
      s.browAngle.target = 0.8 * intensity
      s.eyeY.target = -0.3 * intensity
      s.mouthOpenness = 0.2 * intensity
      break
    case 'scared':
      s.shiverVal = 1.0 * intensity
      s.headY.velocity = 30 * intensity
      s.bodyZ.target = -15 * intensity
      s.eyeX.stiffness = 250 * intensity
      s.eyeY.stiffness = 250 * intensity
      s.browY.target = 0.8 * intensity
      s.browAngle.target = 0.5 * intensity
      s.chest.velocity = 40 * intensity
      break
    case 'determined':
      s.headY.target = 5 * intensity
      s.headZ.target = 5 * intensity
      s.browY.target = -0.6 * intensity
      s.browAngle.target = 0.4 * intensity
      s.eyeX.stiffness = 200 * intensity
      s.chest.target = 10 * intensity
      break
    case 'tired':
      s.headY.target = -15 * intensity
      s.bodyZ.target = -5 * intensity
      s.browY.target = -0.4 * intensity
      s.browAngle.target = -0.2 * intensity
      s.eyeX.stiffness = 40 / intensity
      s.chest.target = -5 * intensity
      sighPose(s)
      break
    case 'shy':
      s.blushVal = 0.6 * intensity
      s.headY.target = -10 * intensity
      s.headZ.target = 15 * intensity
      s.browY.target = 0.2 * intensity
      s.browAngle.target = -0.4 * intensity
      s.hips.target = 5 * intensity
      s.bodyX.target = -5 * intensity
      break
    default:
      break
  }
}
