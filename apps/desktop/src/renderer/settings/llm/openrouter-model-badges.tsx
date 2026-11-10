import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { formatLocalNumber } from '@mika/sdk-ui'
import { ArrowRight, Calendar, Rocket } from 'lucide-react'
import { CpuIcon, EyeIcon, WrenchIcon } from '../../src/components/Icons'
import { useI18n } from '../../src/hooks/useI18n'

export function VendorBadge({ vendor }: { vendor?: string }) {
  const colors: Record<string, string> = {
    openai: 'bg-ok-soft text-ok border-ok-rim/50',
    // The vendor is the OpenRouter id prefix (`meta-llama/...`, `mistralai/...`), not the brand.
    'meta-llama': 'bg-info-soft text-info border-info-rim/50',
    anthropic: 'bg-warn-soft text-warn border-warn-rim/50',
    mistralai: 'bg-info-soft text-info border-info-rim/50',
    google: 'bg-accent-soft text-accent border-accent-rim/50',
    cohere: 'bg-danger-soft text-danger border-danger-rim/50',
    nousresearch: 'bg-accent-soft text-accent border-accent-rim/50',
    sao10k: 'bg-accent-soft text-accent border-accent-rim/50',
    microsoft: 'bg-info-soft text-info border-info-rim/50',
    qwen: 'bg-warn-soft text-warn border-warn-rim/50',
    deepseek: 'bg-info-soft text-info border-info-rim/50',
    'x-ai': 'bg-surface-raise-strong text-text-main border-border-strong',
    'z-ai': 'bg-accent-soft text-accent border-accent-rim/50',
    moonshotai: 'bg-ok-soft text-ok border-ok-rim/50',
  }
  const v = (vendor ?? '').toLowerCase()
  const cls = colors[v] || 'bg-surface-raise-strong text-text-main border-border-strong'
  return (
    <span
      className={`inline-flex items-center rounded-md border px-2 py-0.5 text-micro font-black uppercase tracking-wider ${cls}`}
    >
      {vendor || 'unknown'}
    </span>
  )
}

export function ModelBadges({
  model,
  ctx,
  cutoff,
  created,
  ended,
}: {
  model: LLMModelCatalogItem
  ctx: string
  cutoff: string
  created: string
  ended: string
}) {
  const { t, locale } = useI18n()
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-3.5 pb-2.5">
      {ended && (
        <span
          className="inline-flex items-center gap-1 rounded-md bg-danger-soft px-1.5 py-0.5 text-caption font-semibold text-danger border border-danger-rim/40"
          title={t('settings.openrouter.panel.deactivatedOn')}
        >
          <span className="text-danger/70">⊘</span> {ended}
        </span>
      )}
      {ctx && (
        <span
          className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise/60 px-1.5 py-0.5 text-caption font-semibold text-text-muted"
          title={t('settings.openrouter.model.contextWindow')}
        >
          <CpuIcon className="h-2.5 w-2.5 text-accent" />
          {ctx}
        </span>
      )}
      {model.supportsTools && (
        <span className="inline-flex items-center gap-1 rounded-md border border-info-rim/30 bg-info-soft px-1.5 py-0.5 text-caption font-semibold text-info">
          <WrenchIcon className="h-2.5 w-2.5" /> tools
        </span>
      )}
      {model.supportsVision && (
        <span className="inline-flex items-center gap-1 rounded-md border border-accent-rim/30 bg-accent-soft px-1.5 py-0.5 text-caption font-semibold text-accent">
          <EyeIcon className="h-2.5 w-2.5" /> {t('settings.openrouter.panel.vision')}
        </span>
      )}
      {cutoff && (
        <span
          className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise/40 px-1.5 py-0.5 text-caption text-text-muted"
          title={t('settings.openrouter.panel.knowledgeCutoff')}
        >
          <Calendar aria-hidden="true" className="h-2.5 w-2.5" /> {cutoff}
        </span>
      )}
      {created && (
        <span
          className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise/40 px-1.5 py-0.5 text-caption text-text-muted"
          title={t('settings.openrouter.panel.created')}
        >
          <Rocket aria-hidden="true" className="h-2.5 w-2.5" /> {created}
        </span>
      )}
      {model.maxCompletionTokens ? (
        <span
          className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise/40 px-1.5 py-0.5 text-caption text-text-muted"
          title={t('settings.openrouter.panel.maxOutput')}
        >
          <ArrowRight aria-hidden="true" className="h-2.5 w-2.5" />{' '}
          {formatLocalNumber(model.maxCompletionTokens, locale)}
        </span>
      ) : null}
    </div>
  )
}
