import type { TranslationDict } from '@mika/sdk/i18n'
import { AlertTriangle, Loader2 } from 'lucide-react'
import type { RefObject } from 'react'
import { GroupHeader, PaletteEntry } from './CommandPaletteEntry'
import { type PaletteItem, sharedGroupDescription } from './command-palette-items'

export function CommandPaletteResults({
  listRef,
  t,
  flat,
  grouped,
  query,
  selectedIdx,
  isLoadingTools,
  toolError,
  onRetry,
}: {
  listRef: RefObject<HTMLDivElement | null>
  t: (key: keyof TranslationDict) => string
  flat: PaletteItem[]
  grouped: Array<{ group: string; items: PaletteItem[] }>
  query: string
  selectedIdx: number
  isLoadingTools: boolean
  toolError: string | null
  onRetry: () => void
}) {
  return (
    <div
      ref={listRef}
      className="max-h-80 overflow-y-auto p-2 scroll-py-2"
      role="listbox"
      aria-label={t('shell.palette.results')}
    >
      {flat.length === 0 && !isLoadingTools ? (
        <p role="status" className="py-6 text-center text-[12px] font-medium text-text-muted">
          {t('shell.settingsPalette.noResults')}: &ldquo;{query}&rdquo;
        </p>
      ) : (
        grouped.map((g) => {
          const shared = g.group ? sharedGroupDescription(g.items) : undefined
          return (
            <div key={g.group || '__flat'}>
              {g.group && <GroupHeader label={g.group} note={shared} />}
              {g.items.map((item) => {
                const idx = flat.indexOf(item)
                return (
                  <PaletteEntry
                    key={item.id}
                    item={item}
                    selected={idx === selectedIdx}
                    showGroup={!g.group}
                    hideDescription={shared !== undefined}
                    onSelect={item.onSelect}
                  />
                )
              })}
            </div>
          )
        })
      )}

      {isLoadingTools && (
        <p role="status" className="flex items-center justify-center gap-2 py-3 text-caption text-text-muted">
          <Loader2 size={13} className="animate-spin" aria-hidden="true" />
          {t('shell.palette.loadingTools')}
        </p>
      )}

      {!isLoadingTools && toolError && (
        <p role="alert" className="py-3 text-center text-caption text-danger/80">
          <AlertTriangle aria-hidden="true" size={14} className="inline-block align-text-bottom" /> {toolError}
          <br />
          <button
            type="button"
            onClick={onRetry}
            className="mt-2 text-info hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded"
          >
            {t('button.retry')}
          </button>
        </p>
      )}
    </div>
  )
}
