import type { MatchEffectId } from '../../lib/match-effect'
import { createFireworksScene } from './scene-fireworks'
import type { EffectScene, EffectTier } from './scene-kit'
import { createSakuraScene, createSynthwaveScene } from './scenes-ambient'
import { createConstellationScene, createMatrixScene } from './scenes-digital'

export function createScene(effect: MatchEffectId, tier: EffectTier): EffectScene {
  switch (effect) {
    case 'matrix':
      return createMatrixScene(tier)
    case 'constellation':
      return createConstellationScene(tier)
    case 'sakura':
      return createSakuraScene(tier)
    case 'synthwave':
      return createSynthwaveScene(tier)
    case 'fireworks':
      return createFireworksScene(tier)
  }
}
