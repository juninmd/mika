import { usePluginContext, usePluginTool } from '@mika/sdk-ui'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { rendererLog } from '../../../../renderer/src/lib/renderer-logger'
import { getElectronApi } from '../../../../renderer/src/lib/shell-api'
import { PendingClarifications } from '../components/pending-clarifications'
import type { CardPerson } from '../components/person-card'
import { PersonEmptyState } from '../components/person-empty-state'
import { PersonGallery } from '../components/person-gallery'
import { ListingLoadError, ListingStats, TypeDistribution } from './person-catalog-listing-sections'
import { PersonDetailView } from './person-detail.view'
import type { FullPerson } from './person-detail-parts'

const PERSON_LIST_QUERY = { limit: 200, sortBy: 'lastSeenAt' } as const

export function PersonCatalogListingView() {
  const { t } = useI18n()
  const { api } = usePluginContext()
  const [persons, setPersons] = useState<CardPerson[]>([])
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [filter, setFilter] = useState('all')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detailPerson, setDetailPerson] = useState<FullPerson | null>(null)
  const [detailRefreshToken, setDetailRefreshToken] = useState(0)
  const searchRequestRef = useRef(0)
  const stats = useMemo(() => {
    const types: Record<string, number> = {}
    let enriched = 0
    for (const person of persons) {
      types[person.type] = (types[person.type] ?? 0) + 1
      if (person.enriched) enriched++
    }
    return { total: persons.length, types, enriched }
  }, [persons])

  const {
    data: listData,
    loading: listLoading,
    error: listError,
    refresh: refreshList,
  } = usePluginTool<CardPerson[]>('person_catalog', {
    action: 'list',
    ...PERSON_LIST_QUERY,
  })

  useEffect(() => {
    if (listLoading) return
    setPersons(listData ?? [])
    setLoading(false)
  }, [listData, listLoading])

  useEffect(() => {
    if (!selectedId) {
      setDetailPerson(null)
      return
    }
    setLoading(true)
    void detailRefreshToken
    void api.rpc
      .call<{ success: boolean; data?: FullPerson }>('person_catalog_get', { identifier: selectedId })
      .then((r) => {
        if (r?.success && r.data) setDetailPerson(r.data)
      })
      .catch((error) => {
        // Sem este catch a rejeição do IPC viraria unhandled rejection e a ficha sumiria em silêncio.
        rendererLog.warn('[person-catalog] falha ao carregar a ficha', { error: String(error) })
      })
      .finally(() => setLoading(false))
  }, [selectedId, detailRefreshToken, api.rpc])

  // A ficha muda por enriquecimento em segundo plano ou edição feita no painel de CRUD das
  // Configurações — sem isto esta lista (aberta como janela própria) fica parada até ser fechada
  // e reaberta. Ver review-ledger: era um gap de produto explícito, não um esquecimento.
  useEffect(() => {
    const eAPI = getElectronApi()
    const unsubUpdated = eAPI.runtime.onPersonCatalogUpdated?.((payload) => {
      void refreshList()
      if (payload.personId === selectedId) setDetailRefreshToken((n) => n + 1)
    })
    const unsubDeleted = eAPI.runtime.onPersonCatalogDeleted?.((payload) => {
      void refreshList()
      if (payload.personId === selectedId) setSelectedId(null)
    })
    return () => {
      unsubUpdated?.()
      unsubDeleted?.()
    }
  }, [refreshList, selectedId])

  const handleSearch = useCallback(
    async (q: string) => {
      setSearch(q)
      const requestId = ++searchRequestRef.current
      const trimmed = q.trim()
      setLoading(true)
      let res: { success: boolean; data?: CardPerson[] } | undefined
      try {
        res = trimmed
          ? await api.rpc.call<{ success: boolean; data?: CardPerson[] }>('person_catalog_search', {
              query: q,
              limit: 50,
            })
          : await api.rpc.call<{ success: boolean; data?: CardPerson[] }>('person_catalog_list', PERSON_LIST_QUERY)
      } catch (error) {
        rendererLog.warn('[person-catalog] falha na busca', { error: String(error) })
      } finally {
        if (searchRequestRef.current === requestId) setLoading(false)
      }
      if (searchRequestRef.current !== requestId) return
      if (res?.success && res.data) setPersons(res.data)
    },
    [api.rpc],
  )

  const filtered = useMemo(
    () => (filter === 'all' ? persons : persons.filter((p) => p.type === filter)),
    [filter, persons],
  )
  const typeEntries = useMemo(() => Object.entries(stats.types).sort(([, a], [, b]) => b - a), [stats.types])

  const isEmpty = !loading && !listError && stats.total === 0 && !search.trim() && filter === 'all'

  if (detailPerson) {
    return (
      <main className="min-h-screen bg-bg-deep p-6 text-text-main">
        <PersonDetailView
          person={detailPerson}
          rpc={api.rpc}
          onChanged={() => {
            setDetailRefreshToken((n) => n + 1)
            void refreshList()
          }}
          onClose={() => {
            setSelectedId(null)
            setDetailPerson(null)
          }}
        />
      </main>
    )
  }

  return (
    <main className="min-h-screen bg-bg-deep p-6 text-text-main">
      <div className="max-w-5xl mx-auto space-y-6">
        <div>
          <p className="text-xs font-semibold uppercase tracking-widest text-text-muted">Mika</p>
          <h1 className="text-2xl font-bold">{t('settings.personCatalog.listing.title')}</h1>
          {stats.total > 0 && (
            <p className="text-xs text-text-muted mt-1">
              {stats.total === 1
                ? t('settings.personCatalog.listing.countSingular')
                : `${stats.total} ${t('settings.personCatalog.listing.countPluralSuffix')}`}
              . {t('settings.personCatalog.listing.countFooter')}
            </p>
          )}
        </div>

        {listError && <ListingLoadError listError={listError} refreshList={refreshList} t={t} />}

        <PendingClarifications rpc={api.rpc} onSettled={() => void refreshList()} />

        {!listError && isEmpty && <PersonEmptyState />}

        {!listError && stats.total > 0 && <ListingStats stats={stats} typeEntries={typeEntries} t={t} />}

        {!listError && typeEntries.length > 0 && (
          <TypeDistribution typeEntries={typeEntries} filter={filter} setFilter={setFilter} t={t} />
        )}

        {!listError && !isEmpty && (
          <PersonGallery
            persons={filtered}
            loading={loading}
            search={search}
            onSearchChange={handleSearch}
            filter={filter}
            onFilterChange={setFilter}
            onSelect={setSelectedId}
            rpc={api.rpc}
          />
        )}
      </div>
    </main>
  )
}
