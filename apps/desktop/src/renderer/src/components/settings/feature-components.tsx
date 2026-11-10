import { type ComponentType, lazy } from 'react'
import { rendererLog } from '../../lib/renderer-logger'
import { configStr } from './settings-utils'

export { FEATURE_COMPONENT_HANDLED_FIELDS } from './feature-component-fields'

export type FeatureComponentProps = {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  tabId: string
  pluginId?: string
}

// Each panel is its own chunk: opening Settings used to parse every feature's panel (and their deps)
// up front, though only the active tab renders. `SettingsPanelContent` holds the Suspense boundary.
function lazyPanel<P extends object>(load: () => Promise<ComponentType<P>>) {
  return lazy(async () => ({ default: await load() }))
}

const character = () => import('../../../../features/character/renderer/settings')
const llm = () => import('../../../settings/llm/settings')
const voice = () => import('../../../settings/voice/settings')

const BehaviorPanel = lazyPanel(() => character().then((m) => m.BehaviorPanel))
const CharacterIdentityPanel = lazyPanel(() => character().then((m) => m.CharacterIdentityPanel))
const ThemePickerPanel = lazyPanel(() => character().then((m) => m.ThemePickerPanel))
const ComputerUsePanel = lazyPanel(() =>
  import('../../../../features/computer-use/renderer/settings').then((m) => m.ComputerUsePanel),
)
const GamerCompanionPanel = lazyPanel(() =>
  import('../../../../features/gamer-companion/renderer/settings').then((m) => m.GamerCompanionPanel),
)
const McpPanel = lazyPanel(() => import('../../../../features/mcp/renderer/settings').then((m) => m.McpPanel))
const MoodPanel = lazyPanel(() => import('../../../../features/mood/renderer/settings').then((m) => m.MoodPanel))
const PersonCatalogPanel = lazyPanel(() =>
  import('../../../../features/person-catalog/renderer/settings').then((m) => m.PersonCatalogPanel),
)
const PrivacyPanel = lazyPanel(() =>
  import('../../../../features/privacy/renderer/settings').then((m) => m.PrivacyPanel),
)
const SchedulerPanel = lazyPanel(() =>
  import('../../../../features/scheduler/renderer/settings').then((m) => m.SchedulerPanel),
)
const UserProfilePanel = lazyPanel(() =>
  import('../../../../features/user/renderer/settings').then((m) => m.UserProfilePanel),
)
const MemoryPanel = lazyPanel(() => import('../../../settings/brain/settings').then((m) => m.MemoryPanel))
const BrowserPanel = lazyPanel(() => import('../../../settings/browser/settings').then((m) => m.BrowserPanel))
const EmbeddingProviderPanel = lazyPanel(() => llm().then((m) => m.EmbeddingProviderPanel))
const LLMConnectionPanel = lazyPanel(() => llm().then((m) => m.LLMConnectionPanel))
const OpenRouterModelPanel = lazyPanel(() => llm().then((m) => m.OpenRouterModelPanel))
const TokenUsagePanel = lazyPanel(() => llm().then((m) => m.TokenUsagePanel))
const VisionSettingsPanel = lazyPanel(() =>
  import('../../../settings/vision/settings').then((m) => m.VisionSettingsPanel),
)
const STTProviderPanel = lazyPanel(() => voice().then((m) => m.STTProviderPanel))
const TTSProviderPanel = lazyPanel(() => voice().then((m) => m.TTSProviderPanel))
const LogsPanel = lazyPanel(() => import('./LogsPanel').then((m) => m.LogsPanel))

export const FEATURE_COMPONENTS: Record<string, ComponentType<FeatureComponentProps>> = {
  BehaviorPanel: (props) => <BehaviorPanel {...props} />,
  LogsPanel: (props) => <LogsPanel {...props} />,
  TTSProviderPanel: (props) => <TTSProviderPanel {...props} />,
  STTProviderPanel: (props) => <STTProviderPanel {...props} />,
  OpenRouterModelPanel: (props) => {
    const isOR = configStr(props.config, 'llm.provider') === 'openrouter'
    if (!isOR) return null
    return (
      <OpenRouterModelPanel
        activeModel={configStr(props.config, 'llm.openrouterModel')}
        apiKey={configStr(props.config, 'apiKeys.openrouter')}
        onSave={props.onSave}
      />
    )
  },
  ThemePickerPanel: (props) => <ThemePickerPanel {...props} />,
  LLMConnectionPanel: (props) => <LLMConnectionPanel {...props} />,
  EmbeddingProviderPanel: (props) => <EmbeddingProviderPanel {...props} />,
  McpPanel: (props) => <McpPanel {...props} />,
  UserProfilePanel: (props) => <UserProfilePanel {...props} />,
  CharacterIdentityPanel: (props) => <CharacterIdentityPanel {...props} />,
  GamerCompanionPanel: (props) => <GamerCompanionPanel {...props} />,
  SchedulerPanel: (props) => <SchedulerPanel {...props} />,
  MoodPanel: (props) => <MoodPanel {...props} />,
  PersonCatalogPanel: (props) => <PersonCatalogPanel {...props} />,
  PrivacyPanel: (props) => <PrivacyPanel {...props} />,
  MemoryPanel: (props) => <MemoryPanel {...props} />,
  TokenUsagePanel: (props) => <TokenUsagePanel {...props} />,
  VisionSettingsPanel: (props) => <VisionSettingsPanel {...props} />,
  BrowserPanel: (props) => <BrowserPanel {...props} />,
  ComputerUsePanel: (props) => <ComputerUsePanel {...props} />,
}

export function resolveFeatureComponent(componentName?: string): ComponentType<FeatureComponentProps> | null {
  if (!componentName) return null
  const Component = FEATURE_COMPONENTS[componentName]
  if (Component) {
    rendererLog.debug('[Settings] Resolved settings component', { componentKey: componentName })
    return Component
  }
  return null
}
