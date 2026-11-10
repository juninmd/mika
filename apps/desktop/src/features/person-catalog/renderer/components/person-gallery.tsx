import { AuroraField } from '@mika/sdk-ui/settings'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { type CardPerson, PersonCard, typeLabel } from './person-card'

const FILTERS = [
  'all',
  'politician',
  'real_person',
  'fictional_character',
  'athlete',
  'singer',
  'streamer',
  'vtuber',
  'game',
  'movie',
  'anime',
]

export function PersonGallery({
  persons,
  loading,
  search,
  onSearchChange,
  filter,
  onFilterChange,
  onSelect,
  onDelete,
  rpc,
}: {
  persons: CardPerson[]
  loading: boolean
  search: string
  onSearchChange: (q: string) => void
  filter: string
  onFilterChange: (f: string) => void
  onSelect: (id: string) => void
  onDelete?: (id: string) => void

  rpc?: { call<T>(method: string, args?: unknown): Promise<T> } | null
}) {
  const { t } = useI18n()
  const [localSearch, setLocalSearch] = useState(search)
  const [visible, setVisible] = useState(20)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [])

  const handleSearch = useCallback(
    (q: string) => {
      setLocalSearch(q)
      if (debounceRef.current) clearTimeout(debounceRef.current)
      debounceRef.current = setTimeout(() => onSearchChange(q), 300)
    },
    [onSearchChange],
  )

  const visiblePersons = persons.slice(0, visible)
  const hasMore = visible < persons.length

  const showToolbar = persons.length > 0 || !!search || filter !== 'all'

  return (
    <div className="space-y-3">
      {showToolbar && (
        <>
          <AuroraField
            label={t('settings.personCatalog.gallery.searchPlaceholder')}
            placeholder={t('settings.personCatalog.gallery.searchPlaceholder')}
            type="search"
            hideLabel
            value={localSearch}
            onChange={handleSearch}
          />
          <div className="flex gap-1 flex-wrap">
            {FILTERS.map((f) => (
              <button
                key={f}
                type="button"
                aria-pressed={filter === f}
                onClick={() => onFilterChange(f)}
                className={`text-caption font-bold px-2 py-0.5 rounded-full transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                  filter === f
                    ? 'border border-accent-rim bg-accent-soft text-accent-text'
                    : 'border border-transparent text-text-muted hover:text-text-main'
                }`}
              >
                {f === 'all' ? t('settings.personCatalog.filterAll') : typeLabel(t, f)}
              </button>
            ))}
          </div>
        </>
      )}
      {loading ? (
        <div className="text-xs text-text-muted animate-pulse py-8 text-center">{t('status.loading')}</div>
      ) : visiblePersons.length === 0 ? (
        <div className="text-xs text-text-subtle text-center py-8">
          {search || filter !== 'all'
            ? t('settings.personCatalog.empty.filtered')
            : `${t('settings.personCatalog.empty.title')}. ${t('settings.personCatalog.empty.hint')}`}
        </div>
      ) : (
        <>
          <div className="grid items-start gap-3 grid-cols-2 sm:grid-cols-3 lg:grid-cols-4">
            {visiblePersons.map((p) => (
              <PersonCard key={p.id} person={p} onSelect={onSelect} onDelete={onDelete} rpc={rpc} />
            ))}
          </div>
          {hasMore && (
            <button
              type="button"
              onClick={() => setVisible((v) => v + 20)}
              className="w-full text-caption text-text-subtle hover:text-text-muted transition-colors py-2 font-bold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded-lg"
            >
              {t('settings.personCatalog.gallery.loadMorePrefix')} ({persons.length - visible}{' '}
              {t('settings.personCatalog.gallery.loadMoreSuffix')})
            </button>
          )}
        </>
      )}
    </div>
  )
}
