import { EMBEDDING_TASK_TYPE_OPTIONS } from '@mika/sdk/contracts'
import {
  BentoGrid,
  GlassCard,
  GlassSection,
  GlassSelect,
  GlassSlider,
  GlassToggle,
  ProviderChoiceGrid,
  SectionHeader,
  TestResultNote,
  usePluginTest,
} from '@mika/sdk-ui/settings'
import { Cpu, Globe, Layers, Plug, Server, Sparkles, Zap } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { ModelPickerField } from '../../src/components/settings/ModelPickerField'
import { type BadgeKind, providerBadges } from '../../src/components/settings/provider-badges'
import { configBool, configNum, configStr, pick } from '../../src/components/settings/settings-utils'
import { useProviderModelTransaction } from '../../src/components/settings/useProviderModelTransaction'
import { useI18n } from '../../src/hooks/useI18n'
import { useReportedSave } from '../../src/hooks/useReportedSave'
import { getElectronApi } from '../../src/lib/shell-api'
import {
  buildEmbeddingConfig,
  DEFAULT_MODEL,
  fallbackCatalog,
  PROVIDERS,
  providerDisplayLabel,
} from './embedding-provider-catalog'
import { ProviderChangeConfirm } from './embedding-provider-confirm'
import { LocalEmbeddingCard } from './local-embedding-card'

const EMBEDDING_LOOK: Record<string, { icon: typeof Cpu; badges: BadgeKind[] }> = {
  local: { icon: Cpu, badges: ['recommended', 'local'] },
  google: { icon: Sparkles, badges: ['cloud', 'apiKey'] },
  openrouter: { icon: Globe, badges: ['cloud', 'apiKey'] },
  ollama: { icon: Server, badges: ['local', 'gpu'] },
  openaiCompatible: { icon: Plug, badges: ['apiKey'] },
  unsloth: { icon: Zap, badges: ['local', 'apiKey'] },
}

type Props = {
  embeddingEnabled: boolean
  embeddingProvider: string
  embeddingModel: string
  embeddingTaskType: string
  embeddingOutputDimensionality: number
  onSave: (path: string, value: unknown) => Promise<void>
}

export type EmbeddingProviderPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function EmbeddingProviderPanel(props: EmbeddingProviderPanelProps) {
  const { t } = useI18n()
  const { onSave } = props
  const rawCfg = props.config ?? {}

  const embeddingEnabled = pick(props.embeddingEnabled, () => configBool(rawCfg, 'brain.embeddingEnabled', true))
  const embeddingProvider = pick(props.embeddingProvider, () => configStr(rawCfg, 'llm.embeddingProvider', 'local'))
  const embeddingModel = pick(props.embeddingModel, () => configStr(rawCfg, 'llm.embeddingModel', ''))
  const embeddingTaskType = pick(props.embeddingTaskType, () =>
    configStr(rawCfg, 'llm.embeddingTaskType', 'SEMANTIC_SIMILARITY'),
  )
  const embeddingOutputDimensionality = pick(props.embeddingOutputDimensionality, () =>
    configNum(rawCfg, 'llm.embeddingOutputDimensionality', 0),
  )

  const cfg = buildEmbeddingConfig({
    embeddingProvider,
    embeddingModel,
    embeddingTaskType,
    embeddingOutputDimensionality,
  })

  const fallbackModels = useMemo(() => fallbackCatalog(embeddingProvider), [embeddingProvider])

  const doSave = useReportedSave(onSave)
  const [pendingProviderId, setPendingProviderId] = useState<string | null>(null)

  const applyProviderChange = useProviderModelTransaction({
    onSave,
    providerPath: 'llm.embeddingProvider',
    modelPath: 'llm.embeddingModel',
    currentProvider: embeddingProvider,
    logTag: 'EmbeddingProviderPanel',
  })
  const applyProviderChangeWithDefault = useCallback(
    (id: string) => applyProviderChange(id, DEFAULT_MODEL[id] ?? ''),
    [applyProviderChange],
  )

  const handleProviderChange = useCallback(
    (id: string) => {
      if (id === embeddingProvider) return
      setPendingProviderId(id)
    },
    [embeddingProvider],
  )

  const {
    status: testState,
    message: testMsg,
    runTest: testConnection,
  } = usePluginTest({
    testFn: () => getElectronApi().runtime.testPlugin({ pluginId: 'mika.embeddings', prompt: 'teste' }),
    runningMessage: t('settings.llm.panel.testConnectionRunning'),
    resetDelayMs: 4000,
  })

  const saveBool = onSave as (path: string, value: boolean) => Promise<void>
  const saveNum = onSave as (path: string, value: number) => Promise<void>

  return (
    <GlassSection
      title={t('settings.llm.group.embeddings')}
      icon={Layers}
      description={t('settings.llm.panel.embeddingsDescription')}
      testAction={{ label: t('settings.llm.panel.testConnection'), onTest: testConnection }}
    >
      <BentoGrid cols="auto">
        {testMsg && testState !== 'testing' && (
          <GlassCard span="full">
            <TestResultNote status={testState} message={testMsg} />
          </GlassCard>
        )}
        <GlassCard span="full">
          <GlassToggle
            label={t('settings.llm.field.embeddingEnabled.label')}
            description={t('settings.llm.field.embeddingEnabled.description')}
            configPath="brain.embeddingEnabled"
            value={embeddingEnabled}
            onSave={saveBool}
          />
        </GlassCard>

        <GlassCard span="full">
          <SectionHeader
            title={t('settings.llm.field.provider.label')}
            description={t('settings.embeddings.providerSection.description')}
            className="mb-3"
          />
          <div className="mb-3 empty:hidden">
            <ProviderChangeConfirm
              pendingProviderId={pendingProviderId}
              onCancel={() => setPendingProviderId(null)}
              onConfirm={(id) => {
                void applyProviderChangeWithDefault(id)
                setPendingProviderId(null)
              }}
            />
          </div>
          <ProviderChoiceGrid
            providers={PROVIDERS.map((p) => ({
              ...p,
              label: providerDisplayLabel(p.id, p.label, t),
              icon: EMBEDDING_LOOK[p.id]?.icon,
              badges: providerBadges(t, ...(EMBEDDING_LOOK[p.id]?.badges ?? [])),
            }))}
            activeId={embeddingProvider}
            onSelect={(item) => handleProviderChange(item.id)}
            renderDetails={(item) => (
              <>
                {item.id === 'local' ? (
                  <LocalEmbeddingCard />
                ) : (
                  <ModelPickerField
                    label={t('settings.llm.panel.embeddingModelLabel')}
                    description={t('settings.llm.panel.embeddingModelDescription')}
                    provider={item.id}
                    value={embeddingModel}
                    configPath="llm.embeddingModel"
                    onSave={doSave}
                    kind="embedding"
                    fallbackModels={fallbackModels}
                    fallbackNote={t('settings.llm.panel.embeddingFallbackNote')}
                  />
                )}
                {item.id === 'google' && (
                  <div className="grid gap-4 sm:grid-cols-2">
                    <GlassSelect
                      label={t('settings.llm.field.embeddingTaskType.label')}
                      description={t('settings.llm.field.embeddingTaskType.description')}
                      configPath="llm.embeddingTaskType"
                      options={EMBEDDING_TASK_TYPE_OPTIONS}
                      config={cfg}
                      onSave={doSave}
                    />
                    <GlassSlider
                      label={t('settings.llm.field.embeddingOutputDimensionality.label')}
                      description={t('settings.llm.field.embeddingOutputDimensionality.description')}
                      configPath="llm.embeddingOutputDimensionality"
                      value={embeddingOutputDimensionality}
                      min={0}
                      max={3072}
                      step={1}
                      showValue={(v) => (v === 0 ? t('settings.llm.panel.embeddingOutputDefault') : `${v}`)}
                      onSave={saveNum}
                    />
                  </div>
                )}
              </>
            )}
          />
        </GlassCard>
      </BentoGrid>
    </GlassSection>
  )
}
