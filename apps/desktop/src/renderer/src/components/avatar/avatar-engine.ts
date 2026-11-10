import type { VTuberExpression } from '@mika/sdk/contracts'
import type { VowelWeights } from '../../lib/lip-sync-vowels'

// Shared contract between the Live2D and VRM render backends, picked by model extension in App.tsx.
// Render host lifecycle (PIXI/WebGLRenderer) is intentionally excluded — genuinely different per backend.
export interface AvatarEngineCapabilities {
  // Whether the model moves arms/shoulders/hands beyond the face. Both backends currently declare
  // true (Live2D via Live2DBodyRig, VRM via vrm-body-rig.ts's VRM_BODY_ACTIONS) — kept as a
  // capability flag so a future asymmetry is declared, not silenced (see specs/12-avatar-render.md).
  bodyRig: boolean
  namedActions: boolean
}

export type NumberRef = { current: number }
export type VowelsRef = { current: VowelWeights }

export interface AvatarEngine {
  readonly capabilities: AvatarEngineCapabilities

  // Same signal for both backends (RMS volume + vowel weights from real TTS audio); each maps it to its own params.
  setLipSyncSources(volume?: NumberRef, vowels?: VowelsRef): void

  setExpression(expression: VTuberExpression, intensity?: number): void
  setSpeaking(speaking: boolean): void
  playAction(name: string): void
  setCursorTarget(x: number, y: number): void

  reactToPoke(): void
  // Distinct from reactToPoke(): a click on the avatar's head area (see avatar-hit-area.ts) — a
  // small head-tilt into a happy/shy pose with eyes closed, not a random poke reaction.
  reactToHeadPat(): void

  // Wind strength 0..1 (`windStrengthFromConfig`): VRM spring bones, Live2D hair params.
  setWind(strength: number): void

  dispose(): void
}

// Runtime list for check:avatar-parity; `?.` calls don't fail typecheck if a backend lacks a method.
export const AVATAR_ENGINE_METHODS = [
  'setLipSyncSources',
  'setExpression',
  'setSpeaking',
  'playAction',
  'setCursorTarget',
  'reactToPoke',
  'reactToHeadPat',
  'setWind',
  'dispose',
] as const
