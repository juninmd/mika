import { toDisplayError, type usePluginContext } from '@mika/sdk-ui'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import type { CardPerson } from '../components/person-card'
import type { PersonRecord } from '../person-catalog.types'
import { mergePersons, reportPersonCatalogError } from './person-catalog-crud.helpers'

type PendingMerge = { target: CardPerson; source: CardPerson }
/** Apagar uma pessoa leva junto as memórias que citam o nome: `episodic` é a contagem PREVISTA pelo backend. */
export type PendingDelete = { episodic: number }

type DeleteReply = { success: boolean; data?: { episodicRemoved?: number }; error?: string }

export interface CrudActionDeps {
  api: ReturnType<typeof usePluginContext>['api']
  t: Translate
  editing: PersonRecord | null
  setEditing: (value: PersonRecord | null) => void
  setMode: (value: 'list' | 'edit' | 'new') => void
  load: (silent?: boolean) => Promise<void>
  merging: boolean
  setMerging: (value: boolean) => void
  mergeTarget: CardPerson | null
  setMergeTarget: (value: CardPerson | null) => void
  pendingMerge: PendingMerge | null
  setPendingMerge: (value: PendingMerge | null) => void
  setPendingDelete: (value: PendingDelete | null) => void
}

export function buildCrudActions({
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
}: CrudActionDeps) {
  async function openEdit(id: string) {
    try {
      const res = await api.rpc.call<{ success: boolean; data?: PersonRecord; error?: string }>('person_catalog_get', {
        identifier: id,
      })
      if (res?.success && res.data) {
        setEditing(res.data)
        setMode('edit')
      } else if (!res?.success) {
        reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorEdit'))
      }
    } catch (err) {
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorEdit'))
    }
  }

  async function handleSave(data: Record<string, unknown>) {
    try {
      const res = await api.rpc.call<{ success: boolean; error?: string }>('person_catalog_upsert', {
        ...data,
        ...(editing?.id ? { id: editing.id } : {}),
      })
      if (!res?.success) {
        reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorSave'))
        return
      }
      setMode('list')
      setEditing(null)
      void load()
    } catch (err) {
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorSave'))
    }
  }

  /** Apagar é irreversível e o casamento das memórias é por nome: conta primeiro (`dryRun`), e só pede
   * confirmação quando há memória a perder — sem memória citando a pessoa, apaga direto. */
  async function handleDelete() {
    if (!editing?.id) return

    try {
      const preview = await api.rpc.call<DeleteReply>('person_catalog_delete', { id: editing.id, dryRun: true })
      if (!preview?.success) {
        reportPersonCatalogError(preview?.error, t('settings.personCatalog.crud.errorDelete'))
        return
      }
      const episodic = preview.data?.episodicRemoved ?? 0
      if (episodic > 0) {
        setPendingDelete({ episodic })
        return
      }
      await confirmDelete()
    } catch (err) {
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorDelete'))
    }
  }

  async function confirmDelete() {
    if (!editing?.id) return

    try {
      const res = await api.rpc.call<DeleteReply>('person_catalog_delete', { id: editing.id, confirm: true })
      setPendingDelete(null)
      if (!res?.success) {
        reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorDelete'))
        return
      }
      setMode('list')
      setEditing(null)
      void load()
    } catch (err) {
      setPendingDelete(null)
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorDelete'))
    }
  }

  function handleMergePick(person: CardPerson) {
    if (!merging) return
    if (!mergeTarget) {
      setMergeTarget(person)
      return
    }
    if (mergeTarget.id === person.id) return
    setPendingMerge({ target: mergeTarget, source: person })
  }

  async function confirmMerge() {
    if (!pendingMerge) return
    const { target, source } = pendingMerge
    try {
      const res = await mergePersons(api, target, source)
      if (!res?.success) {
        reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorMerge'))
        setPendingMerge(null)
        setMergeTarget(null)
        setMerging(false)
        return
      }
      setPendingMerge(null)
      setMergeTarget(null)
      setMerging(false)
      void load()
    } catch (err) {
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorMerge'))
      setPendingMerge(null)
      setMergeTarget(null)
      setMerging(false)
    }
  }

  function cancelMerge() {
    setPendingMerge(null)
    setMergeTarget(null)
  }

  function handleCardSelect(person: CardPerson) {
    if (merging) {
      handleMergePick(person)
      return
    }
    void openEdit(person.id)
  }

  async function handleBlock() {
    if (!editing?.id) return
    try {
      const res = await api.rpc.call<{ success: boolean; error?: string }>('person_catalog_block', {
        identifier: editing.id,
        confirm: true,
      })
      if (!res?.success) {
        reportPersonCatalogError(res?.error, t('settings.personCatalog.crud.errorBlock'))
        return
      }
      setMode('list')
      setEditing(null)
      void load()
    } catch (err) {
      reportPersonCatalogError(toDisplayError(err), t('settings.personCatalog.crud.errorBlock'))
    }
  }

  return { handleSave, handleDelete, confirmDelete, handleBlock, handleCardSelect, confirmMerge, cancelMerge }
}
