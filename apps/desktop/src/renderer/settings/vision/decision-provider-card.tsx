import { AuroraField, GlassCard, ProviderChoiceGrid, SectionHeader } from '@mika/sdk-ui/settings'
import { Server, Zap } from 'lucide-react'
import { ModelPickerField } from '../../src/components/settings/ModelPickerField'
import { providerBadges } from '../../src/components/settings/provider-badges'
import type { Translate } from '../../src/hooks/useI18n'

function decisionProviders(t: Translate) {
  return [
    {
      id: 'llamaServer',
      label: t('settings.decision.provider.llamaServer.label'),
      desc: t('settings.decision.provider.llamaServer.desc'),
      icon: Zap,
      badges: providerBadges(t, 'recommended', 'local'),
    },
    {
      id: 'ollama',
      label: t('settings.decision.provider.ollama.label'),
      desc: t('settings.decision.provider.ollama.desc'),
      icon: Server,
      badges: providerBadges(t, 'local', 'gpu'),
    },
  ]
}

/** Which local model decides (`vision.decisionProvider`), with its own model or server field. */
export function DecisionProviderCard({
  t,
  config,
  provider,
  model,
  onSave,
}: {
  t: Translate
  config: Record<string, unknown>
  provider: string
  model: string
  onSave: (path: string, value: unknown) => Promise<void>
}) {
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.vision.field.decisionProvider.label')}
        description={t('settings.decision.providerSection.description')}
        className="mb-3"
      />
      <ProviderChoiceGrid
        providers={decisionProviders(t)}
        activeId={provider}
        onSelect={(p) => void onSave('vision.decisionProvider', p.id)}
        renderDetails={(p) =>
          p.id === 'ollama' ? (
            <ModelPickerField
              label={t('settings.vision.field.decisionModel.label')}
              description={t('settings.vision.field.decisionModel.description')}
              provider="ollama"
              value={model}
              configPath="vision.decisionModel"
              onSave={onSave}
              capability="vision"
            />
          ) : (
            <div className="space-y-2">
              <AuroraField
                label={t('settings.vision.field.decisionBaseUrl.label')}
                description={t('settings.vision.field.decisionBaseUrl.description')}
                configPath="vision.decisionBaseUrl"
                config={config}
                onSave={onSave}
                placeholder="http://127.0.0.1:8080"
              />
              <p className="text-xs text-text-muted break-all">{t('settings.vision.panel.decisionHint')}</p>
            </div>
          )
        }
      />
    </GlassCard>
  )
}
