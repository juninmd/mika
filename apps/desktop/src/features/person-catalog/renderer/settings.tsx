import { toDisplayError } from '@mika/sdk-ui'
import { BentoGrid, GlassCard, GlassSection, GlassSlider, GlassToggle } from '@mika/sdk-ui/settings'
import { ExternalLink, Users } from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool, configNum } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { useUIStore } from '../../../renderer/src/store/useUIStore'

export function PersonCatalogPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const saveBool = onSave as (path: string, value: boolean) => Promise<void>
  const saveNum = onSave as (path: string, value: number) => Promise<void>
  const autoExtract = configBool(config, 'person-catalog.autoExtractOnChat', true)
  const webEnrichment = configBool(config, 'person-catalog.webEnrichment', false)
  const maxMentions = configNum(config, 'person-catalog.maxMentionsPerPerson', 100)
  // Aviso único da migração de config v2→v3: o perfil já existia e tinha a busca web ligada pelo default antigo.
  const showOffNotice = configBool(config, 'person-catalog.webEnrichmentNoticePending', false)

  const openCatalog = async () => {
    const api = getElectronApi()
    try {
      await api.shell.pluginViews?.open({ pluginId: 'mika.person-catalog', viewId: 'person-catalog.listing' })
    } catch (err) {
      useUIStore
        .getState()
        .addNotification('error', toDisplayError(err, t('settings.personCatalog.panel.openCatalogError')))
    }
  }

  return (
    <GlassSection
      title={t('settings.personCatalog.tab.label')}
      icon={Users}
      badge={t('settings.personCatalog.panel.badge')}
    >
      <BentoGrid cols="auto">
        <GlassCard span="full" hover={false}>
          {showOffNotice && (
            <div
              role="status"
              className="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption text-warn/80"
            >
              <p>{t('settings.personCatalog.panel.webEnrichmentOffNotice')}</p>
              <button
                type="button"
                onClick={() => void saveBool('person-catalog.webEnrichmentNoticePending', false)}
                className="shrink-0 rounded-lg border border-border-subtle bg-surface-raise px-3 py-1.5 text-xs font-semibold text-text-main transition-colors hover:border-border-strong"
              >
                {t('settings.personCatalog.panel.webEnrichmentOffNoticeDismiss')}
              </button>
            </div>
          )}
          <div className="grid items-start gap-6 md:grid-cols-2">
            <GlassToggle
              label={t('settings.personCatalog.panel.toggleAutoLabel')}
              description={t('settings.personCatalog.panel.toggleAutoDescription')}
              configPath="person-catalog.autoExtractOnChat"
              value={autoExtract}
              onSave={saveBool}
            />
            <GlassToggle
              label={t('settings.personCatalog.field.webEnrichment.label')}
              description={t('settings.personCatalog.field.webEnrichment.description')}
              configPath="person-catalog.webEnrichment"
              value={webEnrichment}
              onSave={saveBool}
            />
            <GlassSlider
              label={t('settings.personCatalog.panel.sliderLabel')}
              description={t('settings.personCatalog.panel.sliderDescription')}
              configPath="person-catalog.maxMentionsPerPerson"
              value={maxMentions}
              min={1}
              max={500}
              step={10}
              onSave={saveNum}
            />
          </div>
          <div className="mt-5 flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-4">
            <p className="text-xs text-text-muted">{t('settings.personCatalog.panel.privacyNote')}</p>
            <button
              type="button"
              onClick={() => void openCatalog()}
              className="inline-flex items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-xs font-semibold text-text-main transition-colors hover:border-border-strong"
            >
              <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
              {t('settings.personCatalog.panel.openCatalogButton')}
            </button>
          </div>
        </GlassCard>
      </BentoGrid>
    </GlassSection>
  )
}
