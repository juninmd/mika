import { getLLMProviderEntry } from '@mika/sdk/llm/llm-catalog-data'
import { toDisplayError } from '@mika/sdk-ui'
import { BentoGrid, GlassSection } from '@mika/sdk-ui/settings'
import { Cpu } from 'lucide-react'
import { useCallback } from 'react'
import { configBool, configNum, configStr, pick } from '../../src/components/settings/settings-utils'
import { useProviderModelTransaction } from '../../src/components/settings/useProviderModelTransaction'
import { useI18n } from '../../src/hooks/useI18n'
import { rendererLog } from '../../src/lib/renderer-logger'
import { useUIStore } from '../../src/store/useUIStore'
import { LlmAdvancedCard } from './llm-advanced-card'
import { LlmConnectionHero } from './llm-connection-hero'
import { LlmLiveStatus } from './llm-live-status'
import { LlmParamsCard } from './llm-params-card'
import { LLMPromptValidationPanel } from './llm-prompt-validation-panel'
import { LlmProviderCard } from './llm-provider-card'
import type { LlmConnectionProps } from './llm-providers'
import { modelParamLimits } from './model-param-limits'
import { OllamaModelDownload } from './ollama-model-download'
import { useLlmStatus } from './use-llm-status'
import { useModelProfile } from './use-model-profile'

export type LLMConnectionPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<LlmConnectionProps>

const defaultModel = (id: string) => getLLMProviderEntry(id).defaultModel

export function LLMConnectionPanel(props: LLMConnectionPanelProps) {
  const { t } = useI18n()
  const { onSave } = props
  const rawCfg = props.config ?? {}

  const provider = pick(props.provider, () => configStr(rawCfg, 'llm.provider', 'google'))
  const googleModel = pick(props.googleModel, () => configStr(rawCfg, 'llm.googleModel', defaultModel('google')))
  const openrouterModel = pick(props.openrouterModel, () =>
    configStr(rawCfg, 'llm.openrouterModel', defaultModel('openrouter')),
  )
  const ollamaModel = pick(props.ollamaModel, () => configStr(rawCfg, 'llm.ollamaModel', defaultModel('ollama')))
  const openaiCompatibleModel = pick(props.openaiCompatibleModel, () =>
    configStr(rawCfg, 'llm.openaiCompatibleModel', defaultModel('openaiCompatible')),
  )
  const unslothModel = pick(props.unslothModel, () => configStr(rawCfg, 'llm.unslothModel', defaultModel('unsloth')))
  const temperature = pick(props.temperature, () => configNum(rawCfg, 'llm.temperature', 0.7))
  const topP = pick(props.topP, () => configNum(rawCfg, 'llm.topP', 0.95))
  const maxTokens = pick(props.maxTokens, () => configNum(rawCfg, 'llm.maxTokens', 4096))
  const historyLimit = pick(props.historyLimit, () => configNum(rawCfg, 'llm.historyLimit', 20))
  const maxToolSteps = pick(props.maxToolSteps, () => configNum(rawCfg, 'llm.maxToolSteps', 15))
  const contextWindow = pick(props.contextWindow, () => configNum(rawCfg, 'llm.contextWindow', 32768))
  const enableMonologue = pick(props.enableMonologue, () => configBool(rawCfg, 'llm.enableMonologue'))
  const enableSummarization = pick(props.enableSummarization, () => configBool(rawCfg, 'llm.enableSummarization', true))
  const matchUserLanguage = pick(props.matchUserLanguage, () => configBool(rawCfg, 'llm.matchUserLanguage', true))
  const enableThinking = pick(props.enableThinking, () => configBool(rawCfg, 'llm.enableThinking'))
  const thinkingMaxTokens = pick(props.thinkingMaxTokens, () => configNum(rawCfg, 'llm.thinkingMaxTokens', 4096))
  const systemPrompt = pick(props.systemPrompt, () => configStr(rawCfg, 'llm.systemPrompt'))

  const MODEL_PATH_BY_PROVIDER: Record<string, string> = {
    google: 'llm.googleModel',
    openrouter: 'llm.openrouterModel',
    ollama: 'llm.ollamaModel',
    openaiCompatible: 'llm.openaiCompatibleModel',
    unsloth: 'llm.unslothModel',
  }
  const MODEL_VALUE_BY_PROVIDER: Record<string, string> = {
    google: googleModel,
    openrouter: openrouterModel,
    ollama: ollamaModel,
    openaiCompatible: openaiCompatibleModel,
    unsloth: unslothModel,
  }

  const notifySaveError = useCallback(
    (error: unknown) => {
      useUIStore.getState().addNotification('error', toDisplayError(error, t('settings.llm.panel.saveError')))
    },
    [t],
  )

  const doSave = useCallback(
    async (path: string, value: unknown) => {
      try {
        await onSave(path, value)
      } catch (error) {
        rendererLog.error('[LLMConnectionPanel] falha ao salvar', { configPath: path, error: String(error) })
        notifySaveError(error)
      }
    },
    [onSave, notifySaveError],
  )

  const selectProvider = useProviderModelTransaction({
    onSave,
    providerPath: 'llm.provider',
    modelPath: 'llm.model',
    currentProvider: provider,
    logTag: 'LLMConnectionPanel',
  })

  const cfg: Record<string, unknown> = {
    'llm.googleModel': googleModel,
    'llm.openrouterModel': openrouterModel,
    'llm.ollamaModel': ollamaModel,
    'llm.openaiCompatibleModel': openaiCompatibleModel,
    'llm.unslothModel': unslothModel,
    'llm.systemPrompt': systemPrompt,
  }

  // O que o modelo escolhido aceita, pela mesma fonte do turno: esconde e limita os controles.
  const profile = useModelProfile(provider, MODEL_VALUE_BY_PROVIDER[provider] ?? '')
  const limits = modelParamLimits(provider, profile)

  const { status, loading: statusLoading, refresh: refreshStatus } = useLlmStatus()
  // Remounts the model picker when a download lands, so the new model shows up in its list.
  const installedKey = status?.ollama.installed.map((m) => m.name).join('|') ?? ''

  const numSave = onSave as (p: string, v: number) => Promise<void>
  const boolSave = onSave as (p: string, v: boolean) => Promise<void>

  return (
    <GlassSection
      title={t('settings.llm.group.connection')}
      icon={Cpu}
      description={t('settings.llm.panel.description')}
    >
      <BentoGrid cols="auto">
        <LlmConnectionHero
          provider={provider}
          MODEL_VALUE_BY_PROVIDER={MODEL_VALUE_BY_PROVIDER}
          maxTokens={maxTokens}
          maxToolSteps={maxToolSteps}
          live={<LlmLiveStatus status={status} loading={statusLoading} onRefresh={refreshStatus} />}
        />

        <LlmProviderCard
          key={installedKey}
          t={t}
          provider={provider}
          MODEL_PATH_BY_PROVIDER={MODEL_PATH_BY_PROVIDER}
          MODEL_VALUE_BY_PROVIDER={MODEL_VALUE_BY_PROVIDER}
          selectProvider={selectProvider}
          doSave={doSave}
        />

        {provider === 'ollama' && (
          <OllamaModelDownload
            status={status}
            activeModel={ollamaModel}
            onUseModel={(model) => void doSave('llm.ollamaModel', model)}
          />
        )}

        <LlmParamsCard
          t={t}
          temperature={temperature}
          topP={topP}
          maxTokens={maxTokens}
          historyLimit={historyLimit}
          maxToolSteps={maxToolSteps}
          contextWindow={contextWindow}
          limits={limits}
          numSave={numSave}
        />

        <LlmAdvancedCard
          t={t}
          limits={limits}
          enableMonologue={enableMonologue}
          enableSummarization={enableSummarization}
          matchUserLanguage={matchUserLanguage}
          enableThinking={enableThinking}
          thinkingMaxTokens={thinkingMaxTokens}
          cfg={cfg}
          onSave={onSave}
          numSave={numSave}
          boolSave={boolSave}
        />

        <LLMPromptValidationPanel />
      </BentoGrid>
    </GlassSection>
  )
}
