import { AuroraField, GlassCard, GlassSlider, GlassToggle, SectionHeader, StatusPill } from '@mika/sdk-ui/settings'
import { useEffect, useState } from 'react'
import type { LLMProviderHealth } from '../../../shared/electron-api-system-types'
import type { Translate } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'

/** Same key `vision::decision::PROVIDER` records in the provider-health registry. */
const DECISION_PROVIDER = 'decision'
const HEALTH_POLL_MS = 10_000

export function decisionFailure(health: LLMProviderHealth[]): LLMProviderHealth | undefined {
  return health.find((item) => item.provider === DECISION_PROVIDER)
}

function useDecisionFailure(active: boolean): LLMProviderHealth | undefined {
  const [failure, setFailure] = useState<LLMProviderHealth>()
  useEffect(() => {
    if (!active) return
    const load = () =>
      getElectronApi()
        .runtime.getProviderHealth()
        .then(({ health }) => setFailure(decisionFailure(health)))
        .catch(() => setFailure(undefined))
    load()
    const timer = setInterval(load, HEALTH_POLL_MS)
    return () => clearInterval(timer)
  }, [active])
  return failure
}

/** Opt-in decision-model pre-filter for the screen watcher (`vision::decision`). */
export function VisionDecisionCard({
  t,
  config,
  enabled,
  threshold,
  onSave,
}: {
  t: Translate
  config: Record<string, unknown>
  enabled: boolean
  threshold: number
  onSave: (path: string, value: unknown) => Promise<void>
}) {
  const failure = useDecisionFailure(enabled)
  return (
    <GlassCard span="full">
      <div className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <SectionHeader title={t('settings.vision.panel.decisionHeader')} />
          {enabled && (
            <StatusPill
              variant={failure ? 'error' : 'ok'}
              title={failure?.reason}
              label={t(
                failure ? 'settings.vision.panel.decisionStatus.failing' : 'settings.vision.panel.decisionStatus.ok',
              )}
            />
          )}
        </div>
        <GlassToggle
          label={t('settings.vision.field.decisionEnabled.label')}
          description={t('settings.vision.field.decisionEnabled.description')}
          configPath="vision.decisionEnabled"
          value={enabled}
          onSave={onSave as (path: string, value: boolean) => Promise<void>}
        />
        {enabled && (
          <div className="space-y-3 pl-4 border-l border-border-subtle">
            <AuroraField
              label={t('settings.vision.field.decisionBaseUrl.label')}
              description={t('settings.vision.field.decisionBaseUrl.description')}
              configPath="vision.decisionBaseUrl"
              config={config}
              onSave={onSave}
              placeholder="http://127.0.0.1:8080"
            />
            <GlassSlider
              label={t('settings.vision.field.decisionThreshold.label')}
              description={t('settings.vision.field.decisionThreshold.description')}
              configPath="vision.decisionThreshold"
              value={threshold}
              min={0.05}
              max={0.95}
              step={0.05}
              showValue={(v: number) => `${Math.round(v * 100)}%`}
              onSave={onSave as (path: string, value: number) => Promise<void>}
            />
            <p className="text-xs text-text-muted break-all">{t('settings.vision.panel.decisionHint')}</p>
          </div>
        )}
      </div>
    </GlassCard>
  )
}
