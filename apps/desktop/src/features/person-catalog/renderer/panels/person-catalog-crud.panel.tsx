import { AuroraField, GlassSection } from '@mika/sdk-ui/settings'
import { Merge, UserPlus, Users } from 'lucide-react'
import { PendingClarifications } from '../components/pending-clarifications'
import { typeLabel } from '../components/person-card'
import { FILTERS, mergeModeLabel } from './person-catalog-crud.helpers'
import { EditView, MergeConfirmBanner, ResultsSection } from './person-catalog-crud.parts'
import { usePersonCatalogCrud } from './use-person-catalog-crud'

export function PersonCatalogCRUDPanel() {
  const {
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
  } = usePersonCatalogCrud()

  const filterAllLabel = t('settings.personCatalog.filterAll')
  const loadingLabel = t('status.loading')

  if (mode === 'edit' || mode === 'new') {
    return (
      <EditView
        mode={mode}
        editing={editing}
        onSave={handleSave}
        onDelete={handleDelete}
        pendingDelete={pendingDelete}
        onConfirmDelete={confirmDelete}
        onCancelDelete={() => setPendingDelete(null)}
        onBlock={handleBlock}
        onCancel={() => {
          setMode('list')
          setEditing(null)
        }}
        t={t}
      />
    )
  }

  return (
    <div className="space-y-4">
      <PendingClarifications rpc={api.rpc} onSettled={() => void load(true)} />
      <GlassSection
        title={t('settings.personCatalog.crud.title')}
        icon={Users}
        badge={String(filtered.length)}
        description={t('settings.personCatalog.crud.description')}
        actions={
          <>
            <button
              type="button"
              onClick={() => {
                setEditing(null)
                setMode('new')
              }}
              className="inline-flex items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-2.5 py-1.5 text-caption font-bold text-accent-text transition hover:bg-accent/25 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <UserPlus size={13} aria-hidden="true" />
              {t('settings.personCatalog.crud.newPersonButton')}
            </button>
            <button
              type="button"
              onClick={() => {
                setMerging(!merging)
                setMergeTarget(null)
              }}
              className={`inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-caption font-bold transition active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                merging
                  ? 'bg-warn-soft border-warn-rim text-warn'
                  : 'bg-surface-raise border-border-subtle text-text-muted hover:border-border-strong hover:text-text-main'
              }`}
            >
              <Merge size={13} aria-hidden="true" />
              {mergeModeLabel(t, merging, mergeTarget)}
            </button>
          </>
        }
      >
        <div className="space-y-3">
          {merging && (
            <p
              role="status"
              className="text-caption text-warn/80 bg-warn-soft border border-warn-rim rounded-lg px-3 py-2"
            >
              {mergeTarget ? (
                <>
                  {t('settings.personCatalog.crud.mergeStatusKeepingPrefix')} <b>{mergeTarget.primaryName}</b>.{' '}
                  {t('settings.personCatalog.crud.mergeStatusKeepingSuffix')}
                </>
              ) : (
                t('settings.personCatalog.crud.mergeStatusPickBoth')
              )}
            </p>
          )}
          {pendingMerge && (
            <MergeConfirmBanner
              pendingMerge={pendingMerge}
              onCancel={cancelMerge}
              onConfirm={() => void confirmMerge()}
            />
          )}
          <AuroraField
            label={t('settings.personCatalog.crud.searchPlaceholder')}
            placeholder={t('settings.personCatalog.crud.searchPlaceholder')}
            type="search"
            hideLabel
            value={search}
            onChange={setSearch}
          />
          <div className="flex gap-1 flex-wrap">
            {FILTERS.map((f) => (
              <button
                key={f}
                type="button"
                aria-pressed={filter === f}
                onClick={() => setFilter(f)}
                className={`rounded-full px-2.5 py-1 text-caption font-bold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                  filter === f
                    ? 'border border-accent-rim bg-accent-soft text-accent-text'
                    : 'border border-transparent text-text-muted hover:text-text-main'
                }`}
              >
                {f === 'all' ? filterAllLabel : typeLabel(t, f)}
              </button>
            ))}
          </div>
          <ResultsSection
            loading={loading}
            loadingLabel={loadingLabel}
            filtered={filtered}
            persons={persons}
            search={search}
            filter={filter}
            merging={merging}
            mergeTarget={mergeTarget}
            t={t}
            onSelect={handleCardSelect}
          />
        </div>
      </GlassSection>
    </div>
  )
}
