import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, ProviderChoiceGrid, SectionHeader } from '@mika/sdk-ui/settings'
import { Globe, Plug, Server, Sparkles, Zap } from 'lucide-react'
import { ModelPickerField } from '../../src/components/settings/ModelPickerField'
import { type BadgeKind, providerBadges } from '../../src/components/settings/provider-badges'
import type { useProviderModelTransaction } from '../../src/components/settings/useProviderModelTransaction'
import { PROVIDERS, providerDisplayLabel } from './llm-providers'

const LOOK: Record<string, { icon: typeof Globe; badges: BadgeKind[] }> = {
  google: { icon: Sparkles, badges: ['cloud', 'apiKey'] },
  openrouter: { icon: Globe, badges: ['cloud', 'apiKey'] },
  ollama: { icon: Server, badges: ['local', 'gpu'] },
  openaiCompatible: { icon: Plug, badges: ['apiKey'] },
  unsloth: { icon: Zap, badges: ['local', 'apiKey'] },
}

type LlmProviderCardProps = {
  t: (key: keyof TranslationDict) => string
  provider: string
  MODEL_PATH_BY_PROVIDER: Record<string, string>
  MODEL_VALUE_BY_PROVIDER: Record<string, string>
  selectProvider: ReturnType<typeof useProviderModelTransaction>
  doSave: (path: string, value: unknown) => Promise<void>
}

export function LlmProviderCard({
  t,
  provider,
  MODEL_PATH_BY_PROVIDER,
  MODEL_VALUE_BY_PROVIDER,
  selectProvider,
  doSave,
}: LlmProviderCardProps) {
  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.llm.field.provider.label')} className="mb-3" />
      <ProviderChoiceGrid
        providers={PROVIDERS.map((p) => ({
          ...p,
          label: providerDisplayLabel(p.id, p.label, t),
          icon: LOOK[p.id]?.icon,
          badges: providerBadges(t, ...(LOOK[p.id]?.badges ?? [])),
        }))}
        activeId={provider}
        onSelect={(item) => {
          void selectProvider(item.id, MODEL_VALUE_BY_PROVIDER[item.id] ?? '')
        }}
        renderDetails={(item) =>
          item.id === 'openrouter' ? (
            // Catálogo do OpenRouter tem seção própria (busca por preço/visão/tools) logo abaixo —
            // duplicar o ModelPickerField genérico aqui dava dois seletores pro mesmo llm.openrouterModel.
            <p className="text-caption text-text-muted">{t('settings.llm.panel.modelPickedInCatalogBelow')}</p>
          ) : (
            <ModelPickerField
              label={t('settings.llm.panel.modelLabel')}
              description={t('settings.llm.panel.modelDescription')}
              provider={item.id}
              value={MODEL_VALUE_BY_PROVIDER[item.id] ?? ''}
              configPath={MODEL_PATH_BY_PROVIDER[item.id] ?? 'llm.model'}
              onSave={doSave}
              capability="tools"
            />
          )
        }
      />
    </GlassCard>
  )
}
