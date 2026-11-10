import { GlassCard } from '@mika/sdk-ui/settings'
import type { ReactNode } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { PROVIDERS, providerDisplayLabel } from './llm-providers'

function LlmFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2">
      <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">{label}</p>
      <p className="mt-1 truncate text-sm font-semibold text-text-main">{value}</p>
    </div>
  )
}
type LlmConnectionHeroProps = {
  provider: string
  MODEL_VALUE_BY_PROVIDER: Record<string, string>
  maxTokens: number
  maxToolSteps: number
  /** "Status ao vivo" strip, under the facts of what is configured. */
  live: ReactNode
}

/** What is in use right now; the section header above already carries the title and description. */
export function LlmConnectionHero({
  provider,
  MODEL_VALUE_BY_PROVIDER,
  maxTokens,
  maxToolSteps,
  live,
}: LlmConnectionHeroProps) {
  const { t } = useI18n()
  const activeProvider = PROVIDERS.find((p) => p.id === provider)
  const providerLabel = activeProvider ? providerDisplayLabel(activeProvider.id, activeProvider.label, t) : provider
  return (
    <GlassCard span="full" hover={false}>
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
        <LlmFact label={t('settings.llm.hero.provider')} value={providerLabel} />
        <LlmFact
          label={t('settings.llm.hero.activeModel')}
          value={MODEL_VALUE_BY_PROVIDER[provider] || t('settings.llm.hero.providerDefault')}
        />
        <LlmFact
          label={t('settings.llm.hero.maxResponse')}
          value={t('settings.llm.hero.tokens').replace('{{count}}', String(maxTokens))}
        />
        <LlmFact
          label={t('settings.llm.hero.toolsPerTurn')}
          value={t('settings.llm.hero.toolSteps').replace('{{count}}', String(maxToolSteps))}
        />
      </div>
      <div className="mt-4 border-t border-border-subtle pt-4">{live}</div>
    </GlassCard>
  )
}
