import { toDisplayError, usePluginContext } from '@mika/sdk-ui'
import { useCallback, useDeferredValue, useEffect, useMemo, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../../renderer/src/lib/shell-api'
import type { CardPerson } from '../components/person-card'
import type { PersonRecord } from '../person-catalog.types'
import { buildCrudActions, type PendingDelete } from './person-catalog-crud.actions'
import { CATALOG_REFRESH_MS, reportPersonCatalogError, signatureOf } from './person-catalog-crud.helpers'

/** Estado + ações do painel, separado do JSX só para tirar a complexidade cognitiva do componente do teto do Biome. */
export function usePersonCatalogCrud() {
  const { t } = useI18n()
  const { api } = usePluginContext()
  const [persons, setPersons] = useState<CardPerson[]>([])
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [filter, setFilter] = useState('all')
  const [mode, setMode] = useState<'list' | 'edit' | 'new'>('list')
  const [editing, setEditing] = useState<PersonRecord | null>(null)
  const [merging, setMerging] = useState(false)
  const [mergeTarget, setMergeTarget] = useState<CardPerson | null>(null)
  const [pendingMerge, setPendingMerge] = useState<{ target: CardPerson; source: CardPerson } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null)

  const load = useCallback(
    async (silent = false) => {
      if (!silent) setLoading(true)
      try {
        const res = await api.rpc.call<{ success: boolean; data?: CardPerson[]; error?: string }>(
          'person_catalog_list',
          { limit: 300, sortBy: 'lastSeenAt' },
        )
        if (res?.success && res.data) {
          const next = res.data

          setPersons((current) => (signatureOf(current) === signatureOf(next) ? current : next))
        } else if (!res?.success) {
          reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorLoad'))
        }
      } catch (err) {
        reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorLoad'))
      } finally {
        if (!silent) setLoading(false)
      }
    },
    [api.rpc, t],
  )

  useEffect(() => {
    void load()
  }, [load])

  useEffect(() => {
    if (mode !== 'list' || merging) return
    const timer = setInterval(() => {
      if (!document.hidden) void load(true)
    }, CATALOG_REFRESH_MS)
    return () => clearInterval(timer)
  }, [load, mode, merging])

  // Enriquecimento em segundo plano e edições feitas em outra janela (ex: a visão detectando
  // alguém, o painel de listagem) mudam o catálogo sem passar por handleSave/handleDelete daqui —
  // sem isto, esta lista só via a mudança até 3 s depois (o polling acima), ou nunca se não estava em foco.
  useEffect(() => {
    const eAPI = getElectronApi()
    const unsubUpdated = eAPI.runtime.onPersonCatalogUpdated?.(() => void load(true))
    const unsubDeleted = eAPI.runtime.onPersonCatalogDeleted?.(() => void load(true))
    return () => {
      unsubUpdated?.()
      unsubDeleted?.()
    }
  }, [load])

  const { handleSave, handleDelete, confirmDelete, handleBlock, handleCardSelect, confirmMerge, cancelMerge } =
    buildCrudActions({
      api,
      t,
      editing,
      setEditing,
      setMode,
      load,
      merging,
      setMerging,
      mergeTarget,
      setMergeTarget,
      pendingMerge,
      setPendingMerge,
      setPendingDelete,
    })

  const deferredSearch = useDeferredValue(search)
  const filtered = useMemo(() => {
    const q = deferredSearch.toLowerCase()
    return persons.filter((p) => {
      const matchType = filter === 'all' || p.type === filter
      return matchType && (!q || p.primaryName.toLowerCase().includes(q) || (p.origin ?? '').toLowerCase().includes(q))
    })
  }, [persons, filter, deferredSearch])

  return {
    t,
    persons,
    loading,
    search,
    setSearch,
    filter,
    setFilter,
    mode,
    setMode,
    editing,
    setEditing,
    merging,
    setMerging,
    mergeTarget,
    setMergeTarget,
    pendingMerge,
    pendingDelete,
    setPendingDelete,
    filtered,
    handleSave,
    handleDelete,
    confirmDelete,
    handleBlock,
    handleCardSelect,
    confirmMerge,
    cancelMerge,
    api,
    load,
  }
}
