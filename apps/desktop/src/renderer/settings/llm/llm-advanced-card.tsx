import type { TranslationDict } from '@mika/sdk/i18n'
import { AuroraField, GlassCard, GlassSlider, GlassToggle, SectionHeader } from '@mika/sdk-ui/settings'
import type { ModelParamLimits } from './model-param-limits'

type LlmAdvancedCardProps = {
  t: (key: keyof TranslationDict) => string
  limits: ModelParamLimits
  enableMonologue: boolean
  enableSummarization: boolean
  matchUserLanguage: boolean
  enableThinking: boolean
  thinkingMaxTokens: number
  cfg: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  numSave: (p: string, v: number) => Promise<void>
  boolSave: (p: string, v: boolean) => Promise<void>
}

export function LlmAdvancedCard({
  t,
  limits,
  enableMonologue,
  enableSummarization,
  matchUserLanguage,
  enableThinking,
  thinkingMaxTokens,
  cfg,
  onSave,
  numSave,
  boolSave,
}: LlmAdvancedCardProps) {
  const showThinkingBudget = enableThinking && limits.showThinkingBudget
  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.llm.panel.advancedHeader')} className="mb-3" />
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="space-y-3">
          <GlassToggle
            label={t('settings.llm.field.enableMonologue.label')}
            description={t('settings.llm.field.enableMonologue.description')}
            configPath="llm.enableMonologue"
            value={enableMonologue}
            onSave={boolSave}
          />
          <GlassToggle
            label={t('settings.llm.field.enableSummarization.label')}
            description={t('settings.llm.field.enableSummarization.description')}
            configPath="llm.enableSummarization"
            value={enableSummarization}
            onSave={boolSave}
          />
          <GlassToggle
            label={t('settings.llm.field.matchUserLanguage.label')}
            description={t('settings.llm.field.matchUserLanguage.description')}
            configPath="llm.matchUserLanguage"
            value={matchUserLanguage}
            onSave={boolSave}
          />
          {limits.showThinking && (
            <GlassToggle
              label={t('settings.llm.field.enableThinking.label')}
              description={t('settings.llm.field.enableThinking.description')}
              configPath="llm.enableThinking"
              value={enableThinking}
              onSave={boolSave}
            />
          )}
        </div>
        <div className="space-y-4">
          {showThinkingBudget && (
            <GlassSlider
              label={t('settings.llm.field.thinkingMaxTokens.label')}
              description={t('settings.llm.field.thinkingMaxTokens.description')}
              configPath="llm.thinkingMaxTokens"
              value={Math.min(thinkingMaxTokens, limits.thinkingBudgetMax)}
              min={256}
              max={limits.thinkingBudgetMax}
              step={512}
              showValue={(v) => `${v}`}
              onSave={numSave}
            />
          )}
          <AuroraField
            label={t('settings.llm.panel.systemPromptOverrideLabel')}
            configPath="llm.systemPrompt"
            config={cfg}
            onSave={onSave}
            type="textarea"
            rows={showThinkingBudget ? 4 : 7}
            placeholder={t('settings.llm.panel.systemPromptOverridePlaceholder')}
          />
        </div>
      </div>
    </GlassCard>
  )
}
