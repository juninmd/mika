import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { PageHero, type PillVariant, StatusPill } from '@mika/sdk-ui/settings'
import { ChevronLeft } from 'lucide-react'
import { memo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { FeatureValidationButton, FeatureValidationResult, testedModel } from './FeatureValidation'
import { PluginGlyph, resolvePluginVisual } from './plugin-visuals'
import { SettingsFieldCard } from './SettingsFieldCard'
import { getByPath } from './settings-utils'
import { useFeatureValidation } from './useFeatureValidation'

type SettingsTabHeroProps = {
  active: FeatureSettingsDeclaration
  config: Record<string, unknown>

  health: { status: string } | null
  onSave: (path: string, value: unknown) => Promise<void>
  onBack?: () => void
}

type TranslationKey = Parameters<ReturnType<typeof useI18n>['t']>[0]

// "Always on" is a fixed fact, not a state: status colors are reserved for on/off.
function resolveFeatureBadge(
  enabledConfigPath: string | undefined,
  enabled: boolean,
): { labelKey: TranslationKey; variant: PillVariant } {
  if (!enabledConfigPath) return { labelKey: 'settings.tabHero.alwaysOn', variant: 'neutral' }
  return enabled
    ? { labelKey: 'settings.tabHero.active', variant: 'ok' }
    : { labelKey: 'settings.tabHero.inactive', variant: 'inactive' }
}

export const SettingsTabHero = memo(function SettingsTabHero({ active, config, onSave, onBack }: SettingsTabHeroProps) {
  const { t } = useI18n()
  const statePath = active.enabledConfigPath
  const featureEnabled = statePath ? Boolean(getByPath(config, statePath)) : true
  const featureBadge = resolveFeatureBadge(statePath, featureEnabled)
  const visual = resolvePluginVisual(active)
  const validation = useFeatureValidation(active)

  const glyph = visual.showMonogram ? (
    <span className="text-micro font-bold uppercase tracking-widest">{visual.initials}</span>
  ) : (
    <PluginGlyph iconName={visual.iconName} className="h-5 w-5" />
  )

  return (
    <PageHero
      headingId="tab-hero-heading"
      icon={glyph}
      title={active.tabLabel}
      description={active.tabDescription}
      note={active.heroNote}
      pills={
        <>
          {active.tabGroup && <StatusPill variant="neutral" label={active.tabGroup} />}
          <StatusPill variant={featureBadge.variant} label={t(featureBadge.labelKey)} dot />
        </>
      }
      leading={
        onBack && (
          <button
            type="button"
            onClick={onBack}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border-subtle text-text-muted transition-colors hover:border-border-strong hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            title={t('settings.tabHero.back')}
            aria-label={t('settings.tabHero.backToModules')}
          >
            <ChevronLeft className="h-4 w-4" aria-hidden="true" />
          </button>
        )
      }
      actions={
        <>
          <FeatureValidationButton feature={active} validation={validation} />
          {active.enabledConfigPath && (
            <div className="flex items-center gap-2 border-l border-border-subtle pl-3">
              <span className="text-xs font-medium text-text-muted">
                {featureEnabled ? t('settings.tabHero.on') : t('settings.tabHero.off')}
              </span>
              <SettingsFieldCard
                field={{
                  configPath: active.enabledConfigPath,
                  type: 'boolean',
                  label: active.tabLabel,
                  description: '',
                }}
                value={getByPath(config, active.enabledConfigPath)}
                onSave={onSave}
                compact
              />
            </div>
          )}
        </>
      }
    >
      <FeatureValidationResult
        state={validation.state}
        model={testedModel(active.tabId, config)}
        onDismiss={validation.reset}
      />
    </PageHero>
  )
})
