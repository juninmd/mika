import type { VowelWeights } from '../../lib/lip-sync-vowels'
import { rendererLog } from '../../lib/renderer-logger'
import { LIVE2D_ACTION_CATALOG } from './live2d-action-catalog.generated'
import { type ActionTable, pickAvailableAction, startAction, updateActiveAction } from './live2d-animator-actions'
import { applyPhysicsWind, applyToModel, readModelParams } from './live2d-animator-apply'
import { updateBlink } from './live2d-animator-blink'
import {
  BLINK_INTERVAL_MAX,
  BLINK_INTERVAL_MIN,
  IDLE_MOTION_MAX_INTERVAL,
  IDLE_MOTION_MIN_INTERVAL,
  type Mood,
  randomBetween,
} from './live2d-animator-config'
import * as gestures from './live2d-animator-gestures'
import { type IdleHooks, updateIdleMotions } from './live2d-animator-idle'
import { probeParameters, updateLipSync } from './live2d-animator-lip-sync'
import { updateBreathing, updateFocus, updatePhysics, updateSaccades } from './live2d-animator-motion'
import { applyMoodSprings, prepareFreeze, releaseFreeze } from './live2d-animator-pose'
import { applyReactionPose } from './live2d-animator-reactions'
import { createAnimatorState } from './live2d-animator-state'
import { updateProceduralTimers } from './live2d-animator-timers'
import { getMoodForExpression, getReactionActionNames, shouldLeanForwardForExpression } from './live2d-expression'
import type { VowelParams } from './live2d-lip-sync-controller'
import type { Live2DModelInstance } from './live2d-loader'
import { clamp, frameSubsteps } from './live2d-physics'

export type { Mood }

export class Live2DAnimator {
  private model: Live2DModelInstance
  private readonly s = createAnimatorState()
  private readonly actions: ActionTable
  private readonly react = (expression: string) => this.triggerReactionMotion(expression)
  private readonly idleHooks: IdleHooks = {
    react: this.react,
    shrug: () => this.triggerGesture('shrug'),
    playAnyAction: () => this.playFirstAvailableAction(this.listActions(), 1),
  }

  constructor(model: Live2DModelInstance, modelStem?: string) {
    this.model = model
    this.actions = (modelStem ? LIVE2D_ACTION_CATALOG[modelStem] : undefined) ?? {}
    this.s.nextBlinkTime = randomBetween(BLINK_INTERVAL_MIN, BLINK_INTERVAL_MAX)
    this.s.nextIdleTime = randomBetween(IDLE_MOTION_MIN_INTERVAL / 1000, IDLE_MOTION_MAX_INTERVAL / 1000)

    if (modelStem && Object.keys(this.actions).length === 0) {
      rendererLog.warn('[Live2D] skin sem ações no catálogo — reações ficam só no procedural', {
        skin: modelStem,
        conhecidas: Object.keys(LIVE2D_ACTION_CATALOG),
      })
    }
  }

  public getMouthOpenParams(): string[] {
    return this.s.mouthOpenParams
  }

  start(): void {
    probeParameters(this.s, this.model)
    this.s.running = true
  }

  stop(): void {
    this.s.running = false
  }

  dispose(): void {
    this.stop()
    if (this.s.delayedIdleMotionTimer !== null) {
      clearTimeout(this.s.delayedIdleMotionTimer)
      this.s.delayedIdleMotionTimer = null
    }
  }

  update(rawDt: number): void {
    if (!this.s.running) return
    // Throttled (eco 15 fps) frames used to be clamped to 1/30 s: she breathed and blinked at half speed.
    const { steps, step } = frameSubsteps(rawDt)
    for (let i = 0; i < steps; i++) this.step(step)
  }

  private step(dt: number): void {
    this.s.time += dt

    updateActiveAction(this.s, dt)
    updateProceduralTimers(this.s, dt)
    updateBreathing(this.s, dt)
    updateSaccades(this.s, dt)
    updateFocus(this.s, dt)
    updatePhysics(this.s, dt)
    updateBlink(this.s, dt)
    updateLipSync(this.s, this.model, dt)
    updateIdleMotions(this.s, this.idleHooks, dt)

    applyToModel(this.s, this.model.internalModel.coreModel, dt)
    applyPhysicsWind(this.s, this.model.internalModel.physics)
  }

  public setVolumeRef(ref: { current: number }) {
    this.s.volumeRef = ref
  }

  public setVowelsRef(ref: { current: VowelWeights }) {
    this.s.vowelsRef = ref
  }

  public getVowelParams(): VowelParams {
    return this.s.vowelParams
  }

  public setMouthOpenness(v: number) {
    this.s.mouthOpenness = v
  }

  public setSpeaking(speaking: boolean) {
    this.s.isSpeaking = speaking
  }

  public setWind(strength: number) {
    this.s.windStrength = strength
  }

  public setMood(mood: Mood) {
    applyMoodSprings(this.s, mood)
  }

  public setCursorTarget(x: number, y: number) {
    this.s.focusTarget.x = clamp(x, -1, 1)
    this.s.focusTarget.y = clamp(y, -1, 1)
    this.s.hasFocus = true
  }

  public triggerGesture(gestureType: 'greet' | 'shrug' | 'agree'): void {
    this.playFirstAvailableAction(gestures.GESTURE_ACTIONS[gestureType] ?? [], 3)
  }

  public triggerReactionMotion(expression: string, intensity = 1.0): void {
    this.s.currentExpression = expression
    this.playFirstAvailableAction(getReactionActionNames(expression), 2)

    this.setMood(getMoodForExpression(expression))
    this.s.nerdVal = shouldLeanForwardForExpression(expression) ? 1.0 * intensity : 0
    applyReactionPose(this.s, expression, intensity)
  }

  public nod = () => gestures.nodPose(this.s)
  public jump = () => gestures.jumpPose(this.s, this.react)
  public headPat = () => gestures.headPatPose(this.s, this.react)

  public setProbeOverride(overrides: Record<string, number> | null): void {
    this.s.probeOverride = overrides
  }

  public readParams(ids: string[]): Record<string, number> {
    return readModelParams(this.model.internalModel.coreModel, ids)
  }

  public listActions(): string[] {
    return Object.keys(this.actions)
  }

  public playAction(name: string, priority = 2): boolean {
    return startAction(this.s, this.model, this.actions, name, priority) !== null
  }

  private playFirstAvailableAction(candidates: string[], priority = 2): boolean {
    const pick = pickAvailableAction(this.s, this.actions, candidates)
    if (pick === null) return false
    return this.playAction(pick, priority)
  }

  public async freeze(seconds: number, actionName?: string): Promise<boolean> {
    prepareFreeze(this.s)
    this.model.internalModel.motionManager?.stopAllMotions()

    const started = actionName ? startAction(this.s, this.model, this.actions, actionName, 3) : null
    if (started) await started
    const played = !actionName || started !== null

    const step = 1 / 60
    for (let elapsed = 0; elapsed < seconds; elapsed += step) {
      this.model.update(step * 1000)
      this.update(step)
    }
    this.s.running = false
    return played
  }

  public unfreeze(): void {
    releaseFreeze(this.s)
  }
}
