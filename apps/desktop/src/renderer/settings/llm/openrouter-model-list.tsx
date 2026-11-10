import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { useCallback, useMemo, useRef, useState } from 'react'
import { SearchIcon } from '../../src/components/Icons'
import { useI18n } from '../../src/hooks/useI18n'
import { ModelCard } from './openrouter-panel-components'

const LIST_PAGE_SIZE = 60

export function ModelList({
  models,
  cols,
  loading,
  search,
  activeModel,
  onSelect,
}: {
  models: LLMModelCatalogItem[]
  cols: number
  loading: boolean
  search: string
  activeModel: string
  onSelect: (id: string) => void
}) {
  const { t } = useI18n()
  const [visibleCount, setVisibleCount] = useState(LIST_PAGE_SIZE)

  const prevModelsRef = useRef(models)
  if (prevModelsRef.current !== models) {
    prevModelsRef.current = models
    if (visibleCount !== LIST_PAGE_SIZE) setVisibleCount(LIST_PAGE_SIZE)
  }

  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const el = e.currentTarget
      if (el.scrollTop + el.clientHeight >= el.scrollHeight - 300 && visibleCount < models.length) {
        setVisibleCount((c) => Math.min(c + LIST_PAGE_SIZE, models.length))
      }
    },
    [visibleCount, models.length],
  )

  const visible = useMemo(() => models.slice(0, visibleCount), [models, visibleCount])

  let body: React.ReactNode
  if (loading) {
    body = (
      <div className="flex h-full items-center justify-center text-xs font-bold uppercase tracking-widest text-text-muted">
        <span className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-primary animate-pulse" />
          {t('settings.openrouter.panel.syncing')}
        </span>
      </div>
    )
  } else if (models.length === 0) {
    body = (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
        <SearchIcon className="h-10 w-10 text-text-subtle" />
        <div className="text-xs font-bold uppercase tracking-widest text-text-subtle">
          {search
            ? `${t('settings.openrouter.panel.noResultsFor')} "${search}"`
            : t('settings.openrouter.panel.noModels')}
        </div>
      </div>
    )
  } else {
    body = (
      <>
        <div className={`grid gap-2 ${cols > 1 ? 'grid-cols-2' : 'grid-cols-1'}`}>
          {visible.map((model) => (
            <ModelCard
              key={model.id}
              model={model}
              active={model.id === activeModel}
              onSelect={() => onSelect(model.id)}
            />
          ))}
        </div>
        {visibleCount < models.length && (
          <div className="py-3 text-center text-micro font-bold uppercase tracking-widest text-text-subtle">
            {t('settings.openrouter.panel.scrollForMore')} ({models.length - visibleCount}{' '}
            {t('settings.openrouter.panel.remainingSuffix')})
          </div>
        )}
      </>
    )
  }

  return (
    <div
      onScroll={handleScroll}
      className="h-[440px] overflow-y-auto rounded-2xl border border-border-subtle bg-bg-deep/55 p-3"
    >
      {body}
    </div>
  )
}
