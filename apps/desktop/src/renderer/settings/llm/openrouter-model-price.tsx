import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { Zap } from 'lucide-react'
import { CoinsIcon } from '../../src/components/Icons'
import { useI18n } from '../../src/hooks/useI18n'

export function PriceCell({ label, value, per }: { label: string; value: string; per: string }) {
  return (
    <div className="flex flex-col">
      <span className="text-micro font-bold uppercase tracking-wider text-text-muted">{label}</span>
      <div className="flex items-baseline gap-0.5 mt-0.5">
        <span className="text-[12px] font-black tabular-nums text-text-main">{value}</span>
        <span className="text-caption text-text-muted">{per}</span>
      </div>
    </div>
  )
}

export function ModelPrice({
  model,
  promptPrice,
  completionPrice,
  cacheReadPrice,
  total,
}: {
  model: LLMModelCatalogItem
  promptPrice: string | null
  completionPrice: string | null
  cacheReadPrice: string | null
  total: number
}) {
  const { t } = useI18n()
  const priceColor =
    total < 0.5
      ? 'var(--neural-mint, #34d399)'
      : total < 2
        ? 'var(--neural-cyan, #38bdf8)'
        : 'var(--neural-coral, #fb7185)'
  return (
    <>
      <div className="mx-3.5 mb-3 grid grid-cols-[1fr_1fr_auto] gap-2 rounded-xl border border-border-subtle bg-black/25 p-2.5 backdrop-blur-xs">
        {model.isFree ? (
          <div className="col-span-3 flex items-center justify-center gap-1.5 py-1">
            <CoinsIcon className="h-3.5 w-3.5 text-ok" />
            <span className="text-caption font-bold tracking-tight text-ok">
              {t('settings.openrouter.panel.freeModel')}
            </span>
          </div>
        ) : (
          <>
            <PriceCell label={t('settings.openrouter.model.priceIn')} value={promptPrice || '$0'} per="/1M" />
            <PriceCell label={t('settings.openrouter.model.priceOut')} value={completionPrice || '$0'} per="/1M" />
            <div className="flex flex-col items-end justify-center border-l border-border-subtle pl-2.5">
              <span className="text-micro font-bold uppercase tracking-wider text-text-muted">
                {t('settings.openrouter.panel.total')}
              </span>
              <span className="text-[12px] font-black tabular-nums mt-0.5" style={{ color: priceColor }}>
                ${total.toFixed(4)}
              </span>
            </div>
          </>
        )}
      </div>
      {cacheReadPrice && !model.isFree && (
        <div className="px-3.5 pb-3 -mt-1.5">
          <div className="flex items-center gap-1.5 rounded-lg border border-info-rim/30 bg-info-soft/40 px-2.5 py-1">
            <span className="flex items-center gap-1 text-caption font-medium text-info/80">
              <Zap aria-hidden="true" className="h-3 w-3" />
              {t('settings.openrouter.panel.cache')}
            </span>
            <span className="text-caption font-mono font-bold text-info">{cacheReadPrice}</span>
            <span className="text-caption text-info/60">{t('settings.openrouter.panel.cacheReadSuffix')}</span>
          </div>
        </div>
      )}
    </>
  )
}
