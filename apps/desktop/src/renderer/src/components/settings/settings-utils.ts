import { ConfigKey, type FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { getByPath } from '@mika/sdk-ui'
import type { Translate } from '../../hooks/useI18n'

export function truncate(text: string, max = 48): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text
}

export type LabeledOption = { label: string; value: string }

export function labelOf(options: readonly LabeledOption[], value: string): string {
  return options.find((option) => option.value === value)?.label ?? value
}

export function asrProviderOptions(t: Translate): LabeledOption[] {
  return [
    { label: t('settings.stt.provider.nemotron'), value: 'nemotron' },
    { label: t('settings.stt.provider.localWhisper'), value: 'electron' },
    { label: t('settings.stt.provider.browser'), value: 'browser' },
    { label: t('settings.stt.provider.groqWhisper'), value: 'groq-whisper' },
    { label: 'OpenAI Whisper', value: 'openai-whisper' },
  ]
}

export function asrProviderLabels(t: Translate): readonly LabeledOption[] {
  return [...asrProviderOptions(t), { label: t('settings.stt.provider.localWhisper'), value: 'local-whisper' }]
}

export function compactText(text = '', max = 220): string {
  const clean = text.replace(/\s+/g, ' ').trim()
  return clean.length > max ? `${clean.slice(0, max)}…` : clean
}

export function filterDeclarations(
  declarations: FeatureSettingsDeclaration[],
  search: string,
): FeatureSettingsDeclaration[] {
  const term = search.trim().toLowerCase()
  if (!term) return declarations

  return declarations.filter((tab) => {
    if (tab.tabLabel.toLowerCase().includes(term)) return true
    if (tab.tabDescription?.toLowerCase().includes(term)) return true

    return tab.groups.some((group) =>
      group.fields.some((field) =>
        [field.label, field.description, field.configPath, field.example, ...(field.examples ?? [])]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
          .includes(term),
      ),
    )
  })
}

export function groupDeclarations(
  declarations: FeatureSettingsDeclaration[],
): Record<string, FeatureSettingsDeclaration[]> {
  return declarations.reduce<Record<string, FeatureSettingsDeclaration[]>>((acc, tab) => {
    const key = tab.tabGroup || 'General'
    acc[key] ??= []
    acc[key].push(tab)
    return acc
  }, {})
}

export function getToggleableDeclarations(declarations: FeatureSettingsDeclaration[]): FeatureSettingsDeclaration[] {
  return declarations.filter((tab) => Boolean(tab.enabledConfigPath))
}

export function getBulkTogglePlan(
  declarations: FeatureSettingsDeclaration[],
  config: Record<string, unknown>,
): {
  enabledCount: number
  nextValue: boolean
  paths: string[]
  toggleableCount: number
} {
  const toggleable = getToggleableDeclarations(declarations)
  const paths = toggleable.flatMap((tab) => (tab.enabledConfigPath ? [tab.enabledConfigPath] : []))
  const enabledCount = toggleable.filter((tab) => {
    if (!tab.enabledConfigPath) return false
    return Boolean(getByPath(config, tab.enabledConfigPath))
  }).length

  return {
    enabledCount,
    nextValue: enabledCount !== toggleable.length,
    paths,
    toggleableCount: toggleable.length,
  }
}

export { getByPath } from '@mika/sdk-ui'
export { formatValue, parseValue, validateField } from './settings-field-values'

export function setByPath(obj: Record<string, unknown>, path: string, value: unknown): Record<string, unknown> {
  const keys = path.split('.')
  const result = { ...obj }
  let current = result
  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i] as keyof typeof current
    current[key] = { ...(current[key] as object | undefined) }
    current = current[key] as Record<string, unknown>
  }
  current[keys[keys.length - 1] as keyof typeof current] = value
  return result
}

export function pick<T>(override: T | undefined, read: () => T): T {
  return override ?? read()
}

export function configStr(config: Record<string, unknown>, path: string, fallback = ''): string {
  return String(getByPath(config, path) ?? fallback)
}

export function configNum(config: Record<string, unknown>, path: string, fallback = 0): number {
  return Number(getByPath(config, path) ?? fallback)
}

export function configBool(config: Record<string, unknown>, path: string, fallback = false): boolean {
  const value = getByPath(config, path)
  if (typeof value === 'string') return value === 'true'
  if (typeof value === 'boolean') return value
  return fallback
}

// Mirrors MODEL_KEY_FOR_PROVIDER + getModelForProvider from llm-factory.ts (Node-only, can't import).
// Keep the two in sync if the provider -> model-key mapping changes.
const MODEL_KEY_FOR_PROVIDER: Record<string, string> = {
  google: ConfigKey.LlmGoogleModel,
  openrouter: ConfigKey.LlmOpenrouterModel,
  ollama: ConfigKey.LlmOllamaModel,
  openaiCompatible: ConfigKey.LlmOpenaiCompatibleModel,
  unsloth: ConfigKey.LlmUnslothModel,
}

export function getResolvedModel(config: Record<string, unknown>, provider: string): string {
  const modelKey = MODEL_KEY_FOR_PROVIDER[provider] ?? ConfigKey.LlmModel
  // Sem modelo de fábrica, `llm.model` (o campo do Ollama) mostraria um modelo que o Unsloth não tem.
  if (provider === 'unsloth') return configStr(config, modelKey)
  return configStr(config, modelKey) || configStr(config, ConfigKey.LlmModel)
}

export function configStrArray(config: Record<string, unknown>, path: string, fallback: string[] = []): string[] {
  const raw = getByPath(config, path)
  if (Array.isArray(raw)) return raw
  if (typeof raw === 'string')
    return raw
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
  return fallback
}
