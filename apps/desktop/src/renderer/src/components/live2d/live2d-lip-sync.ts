const MAX_MOUTH_OPENNESS = 1

const EXCITED_LIP_SYNC_MULTIPLIER = 1.25
const HAPPY_LIP_SYNC_MULTIPLIER = 1.15
const INTENSE_LIP_SYNC_MULTIPLIER = 1.1
const SUBDUED_LIP_SYNC_MULTIPLIER = 0.8
const RESERVED_LIP_SYNC_MULTIPLIER = 0.85
const DEFAULT_LIP_SYNC_MULTIPLIER = 1

export function getLipSyncMultiplier(expression: string): number {
  switch (expression) {
    case 'excited':
    case 'laugh':
    case 'panic':
      return EXCITED_LIP_SYNC_MULTIPLIER
    case 'happy':
    case 'love':
      return HAPPY_LIP_SYNC_MULTIPLIER
    case 'angry':
    case 'evil':
    case 'determined':
      return INTENSE_LIP_SYNC_MULTIPLIER
    case 'sad':
    case 'tired':
    case 'bored':
      return SUBDUED_LIP_SYNC_MULTIPLIER
    case 'thinking':
    case 'shy':
      return RESERVED_LIP_SYNC_MULTIPLIER
    default:
      return DEFAULT_LIP_SYNC_MULTIPLIER
  }
}

export function getLipSyncTarget(mouthOpenness: number, volume: number | undefined, expression: string): number {
  if (volume === undefined) return mouthOpenness
  return Math.min(MAX_MOUTH_OPENNESS, volume * getLipSyncMultiplier(expression))
}
