import { BLINK_INTERVAL_MAX, BLINK_INTERVAL_MIN, DOUBLE_BLINK_CHANCE, randomBetween } from './live2d-animator-config'
import type { AnimatorState } from './live2d-animator-state'

function blinkMultipliersForExpression(s: AnimatorState): { intervalMult: number; blinkSpeedMult: number } {
  switch (s.currentExpression) {
    case 'panic':
      return { intervalMult: 0.3, blinkSpeedMult: 2.0 }
    case 'bored':
      return { intervalMult: 1.5, blinkSpeedMult: 0.5 }
    case 'tired':
      return { intervalMult: 1.2, blinkSpeedMult: 0.4 }
    case 'surprised':
      return { intervalMult: 2.0, blinkSpeedMult: 1.5 }
    default:
      return {
        intervalMult: ['evil', 'thinking', 'serious'].includes(s.currentExpression) ? 3.0 : 1.0,
        blinkSpeedMult: 1.0,
      }
  }
}

function advanceClosingBlink(s: AnimatorState, dt: number, actualBlinkSpeed: number): void {
  s.blinkValue -= dt * actualBlinkSpeed
  if (s.blinkValue <= 0) {
    s.blinkValue = 0
    s.blinkState = 'closed'
    s.blinkTimer = 0
  }
}

function advanceClosedBlink(s: AnimatorState): void {
  const closedDuration = s.currentExpression === 'tired' ? 0.2 : 0.05
  if (s.blinkTimer > closedDuration) s.blinkState = 'opening'
}

/** A pálpebra sobe mais devagar do que desce (mesma regra do VRM em vrm-blink.ts). */
const OPEN_SPEED_RATIO = 0.55

function advanceOpeningBlink(s: AnimatorState, dt: number, actualBlinkSpeed: number): void {
  s.blinkValue += dt * actualBlinkSpeed * OPEN_SPEED_RATIO
  if (s.blinkValue >= 1) {
    s.blinkValue = 1
    s.blinkState = 'open'
    s.blinkTimer = 0
    s.nextBlinkTime = s.isDoubleBlink ? 0.1 : randomBetween(BLINK_INTERVAL_MIN, BLINK_INTERVAL_MAX)
    if (s.isDoubleBlink) s.isDoubleBlink = false
  }
}

function advanceBlinkState(s: AnimatorState, dt: number, actualBlinkSpeed: number): void {
  switch (s.blinkState) {
    case 'closing':
      advanceClosingBlink(s, dt, actualBlinkSpeed)
      break
    case 'closed':
      advanceClosedBlink(s)
      break
    case 'opening':
      advanceOpeningBlink(s, dt, actualBlinkSpeed)
      break
    default:
      break
  }
}

export function updateBlink(s: AnimatorState, dt: number) {
  if (s.currentExpression === 'sleep') {
    s.blinkValue = 0
    return
  }

  s.blinkTimer += dt

  const { intervalMult, blinkSpeedMult } = blinkMultipliersForExpression(s)

  if (s.blinkState === 'open' && s.blinkTimer > s.nextBlinkTime * intervalMult) {
    s.blinkState = 'closing'
    s.blinkTimer = 0
    s.isDoubleBlink = Math.random() < DOUBLE_BLINK_CHANCE
  }

  const baseBlinkSpeed = 18.0
  advanceBlinkState(s, dt, baseBlinkSpeed * blinkSpeedMult)
}
