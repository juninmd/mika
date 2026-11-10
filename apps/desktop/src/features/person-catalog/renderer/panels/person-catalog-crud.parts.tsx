import { ConfirmButton, EmptyState } from '@mika/sdk-ui/settings'
import { Loader2, SearchX, Users } from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { type CardPerson, PersonCard } from '../components/person-card'
import type { PersonRecord } from '../person-catalog.types'
import type { PendingDelete } from './person-catalog-crud.actions'
import { PersonForm } from './person-form'

export function MergeConfirmBanner({
  pendingMerge,
  onCancel,
  onConfirm,
}: {
  pendingMerge: { target: CardPerson; source: CardPerson }
  onCancel: () => void
  onConfirm: () => void
}) {
  const { t } = useI18n()
  const { target, source } = pendingMerge
  const mergePrefix = t('settings.personCatalog.crud.mergeConfirmBodyPrefix')
  const mergeMiddle = t('settings.personCatalog.crud.mergeConfirmBodyMiddle')
  return (
    <div
      role="alertdialog"
      aria-label={t('settings.personCatalog.crud.mergeConfirmAriaLabel')}
      className="flex items-center justify-between gap-3 text-caption text-warn/80 bg-warn-soft border border-warn-rim rounded-lg px-3 py-2"
    >
      <p>
        {mergePrefix} <b>{source.primaryName}</b> {mergeMiddle} <b>{target.primaryName}</b>
        {t('settings.personCatalog.crud.mergeConfirmBodySuffix')}
      </p>
      <div className="flex items-center gap-2 shrink-0">
        <button type="button" onClick={onCancel} className="text-text-muted hover:text-text-main text-caption">
          {t('button.cancel')}
        </button>
        <ConfirmButton
          ariaLabel={`${mergePrefix} "${source.primaryName}" ${mergeMiddle} "${target.primaryName}"`}
          label={t('settings.personCatalog.crud.mergeConfirmButtonLabel')}
          onConfirm={onConfirm}
        />
      </div>
    </div>
  )
}

/** Antes de apagar: quantas memórias episódicas a varredura por nome levaria junto (prévia do backend). */
export function DeleteConfirmBanner({
  name,
  pendingDelete,
  onCancel,
  onConfirm,
}: {
  name: string
  pendingDelete: PendingDelete
  onCancel: () => void
  onConfirm: () => void
}) {
  const { t } = useI18n()
  return (
    <div
      role="alertdialog"
      aria-label={t('settings.personCatalog.crud.deleteConfirmAriaLabel')}
      className="mb-3 flex items-center justify-between gap-3 text-caption text-warn/80 bg-warn-soft border border-warn-rim rounded-lg px-3 py-2"
    >
      <p>{t('settings.personCatalog.crud.deleteConfirmBody', { name, count: pendingDelete.episodic })}</p>
      <div className="flex items-center gap-2 shrink-0">
        <button type="button" onClick={onCancel} className="text-text-muted hover:text-text-main text-caption">
          {t('button.cancel')}
        </button>
        <ConfirmButton
          ariaLabel={t('settings.personCatalog.crud.deleteConfirmAriaLabel')}
          label={t('settings.personCatalog.crud.deleteConfirmButtonLabel')}
          onConfirm={onConfirm}
        />
      </div>
    </div>
  )
}

export function EditView({
  mode,
  editing,
  onSave,
  onDelete,
  pendingDelete,
  onConfirmDelete,
  onCancelDelete,
  onBlock,
  onCancel,
  t,
}: {
  mode: 'edit' | 'new'
  editing: PersonRecord | null
  onSave: (data: Record<string, unknown>) => Promise<void>
  onDelete: () => Promise<void>
  pendingDelete: PendingDelete | null
  onConfirmDelete: () => Promise<void>
  onCancelDelete: () => void
  onBlock: () => Promise<void>
  onCancel: () => void
  t: Translate
}) {
  return (
    <div className="rounded-2xl p-4 bg-black/20 backdrop-blur-xl border border-border-subtle">
      <div className="flex items-center gap-2 mb-4">
        <button type="button" onClick={onCancel} className="text-text-muted hover:text-text-main text-xs">
          {t('settings.personCatalog.crud.backButton')}
        </button>
        <h3 className="text-sm font-bold text-text-main ml-1">
          {mode === 'new'
            ? t('settings.personCatalog.crud.newPersonHeading')
            : `${t('settings.personCatalog.crud.editHeadingPrefix')} ${editing?.primaryName}`}
        </h3>
      </div>
      {pendingDelete && editing && (
        <DeleteConfirmBanner
          name={editing.primaryName}
          pendingDelete={pendingDelete}
          onCancel={onCancelDelete}
          onConfirm={() => void onConfirmDelete()}
        />
      )}
      <PersonForm
        initial={editing ?? undefined}
        onSave={onSave}
        onDelete={mode === 'edit' ? onDelete : undefined}
        onBlock={mode === 'edit' ? onBlock : undefined}
        onCancel={onCancel}
      />
    </div>
  )
}

export function ResultsSection({
  loading,
  loadingLabel,
  filtered,
  persons,
  search,
  filter,
  merging,
  mergeTarget,
  t,
  onSelect,
}: {
  loading: boolean
  loadingLabel: string
  filtered: CardPerson[]
  persons: CardPerson[]
  search: string
  filter: string
  merging: boolean
  mergeTarget: CardPerson | null
  t: Translate
  onSelect: (person: CardPerson) => void
}) {
  if (loading) return <EmptyState icon={Loader2} title={loadingLabel} />
  if (filtered.length === 0) {
    const narrowed = Boolean(search) || filter !== 'all'
    return (
      <EmptyState
        icon={narrowed ? SearchX : Users}
        title={narrowed ? t('settings.personCatalog.empty.filtered') : t('settings.personCatalog.empty.title')}
        hint={narrowed ? undefined : t('settings.personCatalog.empty.hint')}
        size="block"
        className="rounded-xl border border-dashed border-border-subtle"
      />
    )
  }
  return (
    <div className="grid grid-cols-3 gap-2 max-h-[420px] overflow-y-auto [scrollbar-width:thin] pr-1">
      {filtered.map((p) => (
        <PersonCard
          key={p.id}
          person={p}
          onSelect={(id) => onSelect(persons.find((x) => x.id === id) ?? p)}
          highlight={merging && mergeTarget?.id === p.id}
        />
      ))}
    </div>
  )
}
