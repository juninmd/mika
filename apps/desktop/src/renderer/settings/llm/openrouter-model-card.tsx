import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { formatLocalDate } from '@mika/sdk-ui'
import { memo } from 'react'
import { CheckIcon } from '../../src/components/Icons'
import { useI18n } from '../../src/hooks/useI18n'
import { ModelBadges, VendorBadge } from './openrouter-model-badges'
import { ModelPrice } from './openrouter-model-price'
import { formatContextWindow, formatKnowledgeCutoff, formatPricePer1M } from './openrouter-panel-utils'

export const ModelCard = memo(function ModelCard({
  model,
  active,
  onSelect,
}: {
  model: LLMModelCatalogItem
  active: boolean
  onSelect: () => void
}) {
  const { t, locale } = useI18n()
  const ctx = formatContextWindow(model)
  const cutoff = formatKnowledgeCutoff(model.knowledgeCutoff, locale)
  const created = model.created
    ? formatLocalDate(model.created * 1000, locale, { month: 'short', year: 'numeric' })
    : ''
  const ended = model.ended ? formatLocalDate(model.ended * 1000, locale, { month: 'short', year: 'numeric' }) : ''
  const promptPrice = formatPricePer1M(model.promptPrice)
  const completionPrice = formatPricePer1M(model.completionPrice)
  const cacheReadPrice = formatPricePer1M(model.inputCacheReadPrice)
  const total = model.isFree ? 0 : (Number(model.promptPrice ?? 0) + Number(model.completionPrice ?? 0)) * 1_000_000

  return (
    <button
      type="button"
      onClick={onSelect}
      className={`group relative w-full text-left rounded-2xl border transition duration-300 overflow-hidden ${
        active
          ? 'border-warn-rim bg-gradient-to-br from-warn-soft/60 via-surface-raise to-surface-raise shadow-[0_0_24px_-6px_color-mix(in_srgb,var(--warn)_20%,transparent)] ring-1 ring-warn-rim/40'
          : 'border-border-subtle bg-surface-raise/50 hover:border-border-strong hover:bg-surface-raise hover:shadow-md'
      }`}
    >
      <div className="flex items-start justify-between gap-3 p-3.5 pb-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={`text-sm font-black tracking-tight truncate ${active ? 'text-warn' : 'text-text-main group-hover:text-text-main'}`}
            >
              {model.name}
            </span>
            <VendorBadge vendor={model.vendor} />
          </div>
          <div className="mt-1 flex items-center gap-2 flex-wrap">
            <span className="truncate font-mono text-caption text-text-muted bg-surface-raise px-1.5 py-0.5 rounded border border-border-subtle/50">
              {model.id}
            </span>
            {model.isFree && (
              <span className="inline-flex items-center gap-1 rounded-md bg-ok-soft px-1.5 py-0.5 text-micro font-black uppercase tracking-wider text-ok border border-ok-rim/50">
                {t('settings.openrouter.panel.free')}
              </span>
            )}
          </div>
        </div>
        <div
          className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-xl border transition duration-300 ${
            active
              ? 'border-warn-rim bg-warn-soft text-warn shadow-xs'
              : 'border-border-subtle bg-surface-raise text-transparent group-hover:border-border-strong group-hover:text-text-muted'
          }`}
        >
          <CheckIcon className="size-3.5" />
        </div>
      </div>
      <ModelBadges model={model} ctx={ctx} cutoff={cutoff} created={created} ended={ended} />
      <ModelPrice
        model={model}
        promptPrice={promptPrice}
        completionPrice={completionPrice}
        cacheReadPrice={cacheReadPrice}
        total={total}
      />
    </button>
  )
})
