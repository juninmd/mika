import {
  BentoGrid,
  GlassCard,
  GlassSection,
  GlassToggle,
  ProviderChoiceGrid,
  SectionHeader,
} from '@mika/sdk-ui/settings'
import { Globe, Search, Shield, Sparkles, Wand2 } from 'lucide-react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { configBool, configStr } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import { BrowserSearchPreview } from './search-preview'

const SEARCH_ENGINES = [
  { id: 'auto', icon: Wand2 },
  { id: 'duckduckgo', icon: Shield },
  { id: 'bing', icon: Search },
  { id: 'yahoo', icon: Sparkles },
] as const

export function BrowserPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const enabled = configBool(config, 'browser.enabled', false)
  const searchEngine = configStr(config, 'browser.searchEngine', 'auto')
  return (
    <GlassSection
      title={t('settings.browser.panel.title')}
      icon={Globe}
      badge={t('settings.browser.panel.badge')}
      description={t('settings.browser.panel.description')}
    >
      <BentoGrid cols="auto">
        <GlassCard span="full" hover={false}>
          <GlassToggle
            label={t('settings.browser.field.enabled.label')}
            description={t('settings.browser.field.enabled.description')}
            configPath="browser.enabled"
            value={enabled}
            onSave={onSave}
          />
        </GlassCard>

        <GlassCard span="full" hover={false}>
          <SectionHeader
            title={t('settings.browser.panel.searchEngineLabel')}
            description={t('settings.browser.searchEngine.sectionDescription')}
            className="mb-3"
          />
          <ProviderChoiceGrid
            providers={SEARCH_ENGINES.map((engine) => ({
              id: engine.id,
              label: t(`settings.browser.field.searchEngine.option.${engine.id}`),
              desc: t(`settings.browser.searchEngine.${engine.id}.desc`),
              icon: engine.icon,
            }))}
            activeId={searchEngine}
            onSelect={(engine) => void onSave('browser.searchEngine', engine.id)}
          />
        </GlassCard>

        <BrowserSearchPreview />
      </BentoGrid>
    </GlassSection>
  )
}
