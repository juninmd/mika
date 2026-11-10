import { AlertTriangle, HelpCircle } from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import { TYPE_ICON, typeLabel } from '../components/person-card'

export function ListingLoadError({
  listError,
  refreshList,
  t,
}: {
  listError: string
  refreshList: () => Promise<void>
  t: Translate
}) {
  return (
    <div role="alert" className="rounded-2xl border border-danger-rim bg-danger-soft px-6 py-12 text-center">
      <AlertTriangle aria-hidden="true" className="mx-auto size-12 text-danger" />
      <h2 className="mt-4 text-base font-bold text-danger">{t('settings.personCatalog.listing.loadErrorTitle')}</h2>
      <p className="mx-auto mt-2 max-w-lg text-xs leading-relaxed text-text-muted">{listError}</p>
      <button
        type="button"
        onClick={() => void refreshList()}
        className="mt-6 rounded-lg border border-danger-rim bg-danger-soft px-4 py-2 text-xs font-semibold text-danger transition hover:bg-danger/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        {t('settings.personCatalog.listing.loadErrorRetry')}
      </button>
    </div>
  )
}

export function ListingStats({
  stats,
  typeEntries,
  t,
}: {
  stats: { total: number; enriched: number }
  typeEntries: [string, number][]
  t: Translate
}) {
  return (
    <div className="grid gap-3 sm:grid-cols-4">
      <div role="status" className="rounded-xl bg-surface-raise/40 backdrop-blur-xl border border-border-subtle p-3">
        <p className="text-micro font-bold text-text-muted uppercase tracking-wider">
          {t('settings.personCatalog.listing.statTotal')}
        </p>
        <p className="text-2xl font-black text-text-main mt-1">{stats.total}</p>
      </div>
      <div role="status" className="rounded-xl bg-surface-raise/40 backdrop-blur-xl border border-border-subtle p-3">
        <p className="text-micro font-bold text-text-muted uppercase tracking-wider">
          {t('settings.personCatalog.listing.statDistinctTypes')}
        </p>
        <p className="text-2xl font-black text-text-main mt-1">{typeEntries.length}</p>
      </div>
      <div role="status" className="rounded-xl bg-surface-raise/40 backdrop-blur-xl border border-border-subtle p-3">
        <p className="text-micro font-bold text-text-muted uppercase tracking-wider">
          {t('settings.personCatalog.listing.statEnriched')}
        </p>
        <p className="text-2xl font-black text-text-main mt-1">{stats.enriched}</p>
      </div>
      <div role="status" className="rounded-xl bg-surface-raise/40 backdrop-blur-xl border border-border-subtle p-3">
        <p className="text-micro font-bold text-text-muted uppercase tracking-wider">
          {t('settings.personCatalog.listing.statMostCommonType')}
        </p>
        <p className="text-lg font-black text-text-main mt-1 truncate">
          {typeEntries[0] ? typeLabel(t, typeEntries[0][0]) : ''}
        </p>
      </div>
    </div>
  )
}

export function TypeDistribution({
  typeEntries,
  filter,
  setFilter,
  t,
}: {
  typeEntries: [string, number][]
  filter: string
  setFilter: (value: string) => void
  t: Translate
}) {
  return (
    <div className="rounded-xl bg-surface-raise/40 backdrop-blur-xl border border-border-subtle p-4">
      <p className="text-micro font-bold text-text-muted uppercase mb-2 tracking-wider">
        {t('settings.personCatalog.listing.distributionTitle')}
      </p>
      <div className="flex flex-wrap gap-2">
        {typeEntries.map(([type, count]) => {
          const Icon = TYPE_ICON[type] ?? HelpCircle
          return (
            <button
              key={type}
              type="button"
              aria-pressed={filter === type}
              onClick={() => setFilter(filter === type ? 'all' : type)}
              className={`flex items-center gap-1.5 text-caption px-2.5 py-1 rounded-full transition font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                filter === type
                  ? 'bg-accent-soft text-accent border border-accent-rim'
                  : 'bg-surface-raise text-text-muted hover:text-text-main border border-transparent'
              }`}
            >
              <Icon aria-hidden="true" className="size-3.5" />
              {typeLabel(t, type)} ({count})
            </button>
          )
        })}
      </div>
    </div>
  )
}
