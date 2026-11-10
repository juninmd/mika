import type { VowelWeights } from '../../lib/lip-sync-vowels'
import { DEFAULT_WIND_STRENGTH, type Vec3 } from '../avatar/vrm-wind'
import type { Mood } from './live2d-animator-config'
import type { Live2DBodyRig } from './live2d-body-rig'
import { Live2DLipSyncController, type VowelParams } from './live2d-lip-sync-controller'
import { PerlinNoise1D, Spring } from './live2d-physics'

export type BlinkState = 'open' | 'closing' | 'closed' | 'opening'

export interface ActiveAction {
  name: string
  params: Set<string>
  remaining: number
}

export interface AnimatorState {
  running: boolean
  time: number
  blinkTimer: number
  nextBlinkTime: number
  idleTimer: number
  nextIdleTime: number
  readonly noise: PerlinNoise1D
  readonly lipSync: Live2DLipSyncController
  delayedIdleMotionTimer: ReturnType<typeof setTimeout> | null
  readonly headX: Spring
  readonly headY: Spring
  readonly headZ: Spring
  readonly bodyX: Spring
  readonly bodyZ: Spring
  readonly eyeX: Spring
  readonly eyeY: Spring
  readonly bodyY: Spring
  readonly chest: Spring
  readonly hips: Spring
  readonly browY: Spring
  readonly browAngle: Spring
  readonly tear: Spring
  currentMood: Mood
  isSpeaking: boolean
  mouthOpenness: number
  volumeRef: { current: number } | null
  vowelsRef: { current: VowelWeights } | null
  vowelParams: VowelParams
  lipSyncTracedAt: number
  currentExpression: string
  focusTarget: { x: number; y: number }
  hasFocus: boolean
  saccadeTimer: number
  nextSaccadeTime: number
  saccadeTarget: { x: number; y: number }
  breathVal: number
  proceduralOffsetX: number
  proceduralOffsetY: number
  proceduralOffsetZ: number
  shiverVal: number
  /** Wind strength 0..1 and this frame's wind (`vrm-wind.ts` field, shared with the VRM backend). */
  windStrength: number
  wind: Vec3
  glitchVal: number
  nerdVal: number
  nodTimer: number
  stretchTimer: number
  sighTimer: number
  laughTimer: number
  nerdTimer: number
  yawnTimer: number
  cringeTimer: number
  dizzyTimer: number
  blinkValue: number
  blinkState: BlinkState
  isDoubleBlink: boolean
  blushVal: number
  bodyRig: Live2DBodyRig | null
  supportedParams: Set<string> | null
  mouthOpenParams: string[]
  mouthFormParam: string | null
  activeAction: ActiveAction | null
  actionDrivesBody: boolean
  deterministic: boolean
  probeOverride: Record<string, number> | null
}

export function createAnimatorState(): AnimatorState {
  return {
    running: false,
    time: 0,
    blinkTimer: 0,
    nextBlinkTime: 0,
    idleTimer: 0,
    nextIdleTime: 0,
    noise: new PerlinNoise1D(),
    lipSync: new Live2DLipSyncController(),
    delayedIdleMotionTimer: null,
    headX: new Spring(0, 80, 6),
    headY: new Spring(0, 80, 6),
    headZ: new Spring(0, 60, 8),
    bodyX: new Spring(0, 40, 8),
    bodyZ: new Spring(0, 40, 8),
    eyeX: new Spring(0, 150, 8),
    eyeY: new Spring(0, 150, 8),
    bodyY: new Spring(0, 50, 7),
    chest: new Spring(0, 30, 6),
    hips: new Spring(0, 30, 8),
    browY: new Spring(0, 100, 8),
    browAngle: new Spring(0, 100, 8),
    tear: new Spring(0, 20, 8, 2),
    currentMood: 'neutral',
    isSpeaking: false,
    mouthOpenness: 0,
    volumeRef: null,
    vowelsRef: null,
    vowelParams: {},
    lipSyncTracedAt: 0,
    currentExpression: 'neutral',
    focusTarget: { x: 0, y: 0 },
    hasFocus: false,
    saccadeTimer: 0,
    nextSaccadeTime: 1.0,
    saccadeTarget: { x: 0, y: 0 },
    breathVal: 0,
    proceduralOffsetX: 0,
    proceduralOffsetY: 0,
    proceduralOffsetZ: 0,
    shiverVal: 0,
    windStrength: DEFAULT_WIND_STRENGTH,
    wind: { x: 0, y: 0, z: 0 },
    glitchVal: 0,
    nerdVal: 0,
    nodTimer: 0,
    stretchTimer: 0,
    sighTimer: 0,
    laughTimer: 0,
    nerdTimer: 0,
    yawnTimer: 0,
    cringeTimer: 0,
    dizzyTimer: 0,
    blinkValue: 1,
    blinkState: 'open',
    isDoubleBlink: false,
    blushVal: 0,
    bodyRig: null,
    supportedParams: null,
    mouthOpenParams: [],
    mouthFormParam: null,
    activeAction: null,
    actionDrivesBody: false,
    deterministic: false,
    probeOverride: null,
  }
}
