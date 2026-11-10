import { HARU_OUTFITS, VRM_LOOKS, vrmModelPath } from '../avatar/vrm-look'

// Default is the bundled VRM; Live2D samples are third-party and excluded from the distributed package (LICENSE-ASSETS.md).
export { DEFAULT_MODEL_PATH } from '../../../../shared/avatar-model'

// Fallback inside the Live2D loader only — a `.vrm` here would break Cubism.
export const FALLBACK_LIVE2D_MODEL_PATH = 'live2d/haru/haru_greeter_t05.model3.json'

export type Live2DModelOption = {
  id: string
  label: string
  path: string
  catalogKey: string

  preview?: string
  /** Zoom (from the top) that turns a full-body preview into the roster card's bust crop. */
  previewZoom?: number

  /** Whether the asset ships in the installer (Live2D samples are excluded, see LICENSE-ASSETS.md). */
  bundled: boolean
}

export const LIVE2D_MODELS: readonly Live2DModelOption[] = [
  {
    id: 'haru',
    label: 'Haru',
    path: FALLBACK_LIVE2D_MODEL_PATH,
    catalogKey: 'haru_greeter_t05',
    preview: 'live2d/haru/preview.png',
    bundled: false,
  },
  {
    id: 'mao',
    label: 'Mao',
    path: 'live2d/Mao/Mao.model3.json',
    catalogKey: 'Mao',
    preview: 'live2d/Mao/preview.png',
    previewZoom: 1.9,
    bundled: false,
  },
]

const VRM_BASE = 'vrm/mika/mika.vrm'

export const VRM_MODELS: readonly Live2DModelOption[] = [...VRM_LOOKS, ...HARU_OUTFITS].map((look) => ({
  id: `vrm-${look.id}`,
  label: `${look.name} (VRM)`,
  path: vrmModelPath(VRM_BASE, look.id),
  catalogKey: `vrm-${look.id}`,
  preview: `vrm/mika/preview-${look.id}.png`,
  bundled: true,
}))

export const AVATAR_MODELS: readonly Live2DModelOption[] = [...LIVE2D_MODELS, ...VRM_MODELS]

type Live2DModelPathInput = {
  modelName?: string | null
  modelPath?: string | null
}

export function resolveLive2DModelPath(input: Live2DModelPathInput): string {
  const modelName = String(input.modelName ?? 'default').trim()
  const modelPath = String(input.modelPath ?? '').trim()

  if (modelPath) {
    return modelPath
  }

  if (modelName && modelName !== 'default') {
    return `live2d/${modelName}/${modelName}.model3.json`
  }

  return FALLBACK_LIVE2D_MODEL_PATH
}
