import { DEFAULT_MODEL_PATH } from '../../../../shared/avatar-model'
import { HARU_OUTFITS, VRM_LOOKS, vrmModelPath } from '../avatar/vrm-look'

// Default is the bundled VRM; Live2D only loads from a user-provided path (no sample ships).
export { DEFAULT_MODEL_PATH }

export type Live2DModelOption = {
  id: string
  label: string
  path: string
  catalogKey: string

  preview?: string
  /** Zoom (from the top) that turns a full-body preview into the roster card's bust crop. */
  previewZoom?: number

  /** Whether the asset ships in the installer. */
  bundled: boolean
}

const VRM_BASE = 'vrm/mika/mika.vrm'

export const VRM_MODELS: readonly Live2DModelOption[] = [...VRM_LOOKS, ...HARU_OUTFITS].map((look) => ({
  id: `vrm-${look.id}`,
  label: `${look.name} (VRM)`,
  path: vrmModelPath(VRM_BASE, look.id),
  catalogKey: `vrm-${look.id}`,
  preview: `vrm/mika/preview-${look.id}.png`,
  bundled: true,
}))

export const AVATAR_MODELS: readonly Live2DModelOption[] = VRM_MODELS

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

  return DEFAULT_MODEL_PATH
}
