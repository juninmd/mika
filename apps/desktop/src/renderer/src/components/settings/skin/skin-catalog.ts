import type { TranslationDict } from '@mika/sdk/i18n'
import { VRM_BODY_ACTIONS } from '../../avatar/vrm-body-rig'
import { EXPRESSION_PRESET, VRM_PRESETS } from '../../avatar/vrm-look'
import { EXPRESSION_ACTION_MAP } from '../../live2d/live2d-animator-config'
import type { Live2DModelOption } from '../../live2d/model-path'

export type SkinKind = 'vrm' | 'live2d'

/** Where a skin's asset came from — mirrors LICENSE-ASSETS.md, which is the source of truth. */
export type SkinSource = 'pixiv' | 'haru' | 'live2d'

export interface SkinFacts {
  kind: SkinKind
  source: SkinSource
  /** Named expressions the `avatar` tool can show on this backend. */
  expressions: number
  /** Body actions (wave, bow, peace…) the backend can play. */
  actions: number
  /** Face presets driven directly by blend shapes (VRM only; Live2D maps via motions). */
  facePresets: readonly string[]
}

const LIVE2D_ACTIONS = new Set(Object.values(EXPRESSION_ACTION_MAP).flat()).size

export function skinFacts(model: Live2DModelOption): SkinFacts {
  if (!model.path.toLowerCase().includes('.vrm')) {
    const expressions = Object.keys(EXPRESSION_ACTION_MAP).length
    return { kind: 'live2d', source: 'live2d', expressions, actions: LIVE2D_ACTIONS, facePresets: [] }
  }
  return {
    kind: 'vrm',
    source: model.path.includes('haru.vrm') ? 'haru' : 'pixiv',
    expressions: Object.keys(EXPRESSION_PRESET).length,
    actions: VRM_BODY_ACTIONS.length,
    facePresets: VRM_PRESETS,
  }
}

export const SOURCE_KEYS: Record<SkinSource, { origin: keyof TranslationDict; license: keyof TranslationDict }> = {
  pixiv: { origin: 'settings.skin.details.source.pixiv', license: 'settings.skin.details.license.pixiv' },
  haru: { origin: 'settings.skin.details.source.haru', license: 'settings.skin.details.license.haru' },
  live2d: { origin: 'settings.skin.details.source.live2d', license: 'settings.skin.details.license.live2d' },
}
