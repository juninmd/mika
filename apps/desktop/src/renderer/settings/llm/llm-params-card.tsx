import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, GlassSlider, SectionHeader, StatTile } from '@mika/sdk-ui/settings'
import { formatContext } from '../../src/components/settings/model-catalog'
import type { ModelParamLimits } from './model-param-limits'

/** Teto do slider quando o Ollama não informou a janela do modelo (o backend ainda corta nela). */
const UNKNOWN_MODEL_CONTEXT_MAX = 131072

type LlmParamsCardProps = {
  t: (key: keyof TranslationDict) => string
  temperature: number
  topP: number
  maxTokens: number
  historyLimit: number
  maxToolSteps: number
  contextWindow: number
  limits: ModelParamLimits
  numSave: (p: string, v: number) => Promise<void>
}

function contextWindowDescription(t: LlmParamsCardProps['t'], modelMax: number | undefined): string {
  const base = t('settings.llm.field.contextWindow.description')
  const max = formatContext(modelMax)
  return max ? `${base} ${t('settings.llm.panel.modelMaxContext')} ${max}.` : base
}

export function LlmParamsCard({
  t,
  temperature,
  topP,
  maxTokens,
  historyLimit,
  maxToolSteps,
  contextWindow,
  limits,
  numSave,
}: LlmParamsCardProps) {
  const contextMax = limits.contextMax ?? UNKNOWN_MODEL_CONTEXT_MAX
  const contextLabel = formatContext(limits.contextMax)
  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.llm.panel.paramsHeader')} className="mb-3" />
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        <GlassSlider
          label={t('settings.llm.field.temperature.label')}
          configPath="llm.temperature"
          value={temperature}
          min={0}
          max={2}
          step={0.05}
          showValue={(v) => v.toFixed(2)}
          onSave={numSave}
        />
        {limits.showTopP && (
          <GlassSlider
            label={t('settings.llm.field.topP.label')}
            configPath="llm.topP"
            value={topP}
            min={0}
            max={1}
            step={0.01}
            showValue={(v) => v.toFixed(2)}
            onSave={numSave}
          />
        )}
        <GlassSlider
          label={t('settings.llm.field.maxTokens.label')}
          configPath="llm.maxTokens"
          value={Math.min(maxTokens, limits.maxTokensMax)}
          min={256}
          max={limits.maxTokensMax}
          showValue={(v) => `${v}`}
          onSave={numSave}
        />
        <GlassSlider
          label={t('settings.llm.field.historyLimit.label')}
          configPath="llm.historyLimit"
          value={historyLimit}
          min={1}
          max={100}
          showValue={(v) => `${v}`}
          onSave={numSave}
        />
        <GlassSlider
          label={t('settings.llm.panel.toolStepsLabel')}
          configPath="llm.maxToolSteps"
          value={maxToolSteps}
          min={1}
          max={50}
          showValue={(v) => `${v}`}
          onSave={numSave}
        />
        {limits.contextEditable ? (
          <GlassSlider
            label={t('settings.llm.field.contextWindow.label')}
            description={contextWindowDescription(t, limits.contextMax)}
            configPath="llm.contextWindow"
            value={Math.min(contextWindow, contextMax)}
            min={2048}
            max={contextMax}
            step={1024}
            showValue={(v) => formatContext(v) ?? `${v}`}
            onSave={numSave}
          />
        ) : (
          contextLabel && <StatTile label={t('settings.llm.panel.modelContextLabel')} value={contextLabel} />
        )}
      </div>
    </GlassCard>
  )
}
