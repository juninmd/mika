import type { PluginUIConfig } from './plugin-ui-loader'

const FEATURE_UI_LOADERS: Record<string, () => Promise<{ default: PluginUIConfig }>> = {
  'mika.brain': () => import('../../settings/brain/index'),
  'mika.chat': () => import('../../settings/chat/index'),
  'mika.vision': () => import('../../settings/vision/index'),
  'mika.voice': () => import('../../settings/voice/index'),
  'mika.character': () => import('../../../features/character/renderer/index'),
  'mika.gamer-companion': () => import('../../../features/gamer-companion/renderer/index'),
  'mika.mood': () => import('../../../features/mood/renderer/index'),
  'mika.person-catalog': () => import('../../../features/person-catalog/renderer/index'),
  'mika.scheduler': () => import('../../../features/scheduler/renderer/index'),
  'mika.user': () => import('../../../features/user/renderer/index'),
}

const cache = new Map<string, Promise<PluginUIConfig>>()

export function loadFeatureUi(featureId: string): Promise<PluginUIConfig> | null {
  const load = FEATURE_UI_LOADERS[featureId]
  if (!load) return null

  const cached = cache.get(featureId)
  if (cached) return cached

  const promise = load()
    .then((module) => module.default ?? {})
    .catch((error) => {
      cache.delete(featureId)
      throw error
    })
  cache.set(featureId, promise)
  return promise
}
