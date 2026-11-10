import type { AnimatorState } from './live2d-animator-state'
import { lerp } from './live2d-physics'

export function updateProceduralTimers(s: AnimatorState, dt: number): void {
  updateNodTimer(s, dt)
  updateStretchTimer(s, dt)
  updateSighTimer(s, dt)
  updateLaughTimer(s, dt)
  updateNerdTimer(s, dt)
  updateYawnTimer(s, dt)
  updateCringeTimer(s, dt)
  updateDizzyTimer(s, dt)
}

function updateNodTimer(s: AnimatorState, dt: number) {
  if (s.nodTimer <= 0) return
  s.nodTimer -= dt
  if (s.nodTimer > 0.3) {
    s.headY.velocity = -25
  } else if (s.nodTimer > 0.18) {
    s.headY.velocity = 25
  } else if (s.nodTimer > 0) {
    s.headY.velocity = -10
  }
}

function updateStretchTimer(s: AnimatorState, dt: number) {
  if (s.stretchTimer <= 0) return
  s.stretchTimer -= dt
  if (s.stretchTimer <= 0) {
    s.bodyY.velocity = -10
    s.chest.velocity = -15
  }
}

function updateSighTimer(s: AnimatorState, dt: number) {
  if (s.sighTimer <= 0) return
  s.sighTimer -= dt
  if (s.sighTimer > 0.4) {
    return
  }
  if (s.sighTimer > 0) {
    s.chest.velocity = -30
    s.bodyY.velocity = -15
    s.headY.velocity = -20
    if (s.sighTimer > 0.2) {
      s.mouthOpenness = lerp(s.mouthOpenness, 0.5, dt * 10)
    } else {
      s.mouthOpenness = lerp(s.mouthOpenness, 0, dt * 10)
    }
  }
}

function updateLaughTimer(s: AnimatorState, dt: number) {
  if (s.laughTimer <= 0) return
  s.laughTimer -= dt
  if (s.laughTimer > 0) {
    const bouncePhase = Math.floor(s.laughTimer * 5) % 2 === 0
    s.bodyY.velocity = bouncePhase ? -15 : 15
    s.chest.velocity = bouncePhase ? -10 : 10
  } else {
    s.mouthOpenness = lerp(s.mouthOpenness, 0, dt * 5)
  }
}

function updateNerdTimer(s: AnimatorState, dt: number) {
  if (s.nerdTimer <= 0) return
  s.nerdTimer -= dt
  if (s.nerdTimer <= 0) {
    s.nerdVal = 0
  }
}

function updateYawnTimer(s: AnimatorState, dt: number) {
  if (s.yawnTimer <= 0) return
  s.yawnTimer -= dt
  if (s.yawnTimer > 0) {
    s.mouthOpenness = lerp(s.mouthOpenness, 1.0, dt * 5)
  } else {
    s.mouthOpenness = lerp(s.mouthOpenness, 0, dt * 5)
  }
}

function updateCringeTimer(s: AnimatorState, dt: number) {
  if (s.cringeTimer <= 0) return
  s.cringeTimer -= dt
  if (s.cringeTimer > 0) {
    s.bodyY.velocity = -20
    s.headY.velocity = -10
    s.headZ.target = lerp(s.headZ.target, 20, dt * 5)
  }
}

function updateDizzyTimer(s: AnimatorState, dt: number) {
  if (s.dizzyTimer <= 0) return
  s.dizzyTimer -= dt
  if (s.dizzyTimer > 0) {
    const dizzySpeed = 10
    s.headX.target += Math.sin(s.time * dizzySpeed) * 15
    s.headY.target += Math.cos(s.time * dizzySpeed) * 15
    s.eyeX.target += Math.sin(s.time * dizzySpeed * 2) * 50
    s.eyeY.target += Math.cos(s.time * dizzySpeed * 2) * 50
  }
}
