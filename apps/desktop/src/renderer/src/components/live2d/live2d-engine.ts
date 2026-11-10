import type { VTuberExpression } from '@mika/sdk/contracts'
import type { AvatarEngine, AvatarEngineCapabilities, NumberRef, VowelsRef } from '../avatar/avatar-engine'
import type { Live2DAnimator } from './live2d-animator'
import type { Live2DModelInstance } from './live2d-loader'

// Live2D side of the AvatarEngine contract. PIXI.Application lifecycle is intentionally excluded —
// the render host is genuinely different between backends.
export class Live2DEngine implements AvatarEngine {
  readonly capabilities: AvatarEngineCapabilities = { bodyRig: true, namedActions: true }

  constructor(
    private readonly model: Live2DModelInstance,
    // Public so the Pixi ticker and debug bridge can talk to the animator directly.
    readonly animator: Live2DAnimator,
  ) {}

  setLipSyncSources(volume?: NumberRef, vowels?: VowelsRef): void {
    if (volume) this.animator.setVolumeRef(volume)
    if (vowels) this.animator.setVowelsRef(vowels)
  }

  // Two things happen per expression: the lib's native swap and the generated reaction motion;
  // without the motion the change is nearly invisible on several models.
  setExpression(expression: VTuberExpression, intensity = 1): void {
    try {
      this.model.expression(expression)
    } catch {
      // Not every model declares every expression; the reaction motion below still applies.
    }
    this.animator.triggerReactionMotion(expression, intensity)
  }

  setSpeaking(speaking: boolean): void {
    this.animator.setSpeaking(speaking)
  }

  playAction(name: string): void {
    this.animator.playAction(name)
  }

  setCursorTarget(x: number, y: number): void {
    this.animator.setCursorTarget(x, y)
  }

  reactToPoke(): void {
    if (Math.random() > 0.5) {
      this.animator.nod()
      this.animator.triggerGesture('greet')
      return
    }
    this.animator.jump()
    this.animator.triggerGesture('agree')
  }

  reactToHeadPat(): void {
    this.animator.headPat()
  }

  setWind(strength: number): void {
    this.animator.setWind(strength)
  }

  dispose(): void {
    this.animator.dispose()
  }
}
