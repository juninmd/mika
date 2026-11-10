import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, ConfirmButton, GlassSection, GlassSelect, StatusPill } from '@mika/sdk-ui/settings'
import { Brain, Check, Merge, Pencil, Trash2, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { type Translate, useI18n } from '../../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../../renderer/src/store/useUIStore'
import type { PersonFact } from '../person-catalog.types'
import type { FullPerson } from './person-detail-parts'

type Rpc = { call<T>(method: string, args?: unknown): Promise<T> }
type RpcResult = { success: boolean; error?: string }

async function runRpc(rpc: Rpc, method: string, args: unknown, t: Translate): Promise<boolean> {
  try {
    const res = await rpc.call<RpcResult>(method, args)
    if (res?.success) return true
    useUIStore.getState().addNotification('error', res?.error ?? t('settings.personCatalog.memory.errorGeneric'))
  } catch (err) {
    useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.personCatalog.memory.errorGeneric')))
  }
  return false
}

function AliasChips({ person, rpc, onChanged }: { person: FullPerson; rpc: Rpc; onChanged: () => void }) {
  const { t } = useI18n()
  if (!person.aliases.length) return null
  const remove = async (alias: string) => {
    if (await runRpc(rpc, 'person_catalog_alias_remove', { personId: person.id, alias }, t)) onChanged()
  }
  return (
    <div>
      <p className="mb-1.5 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.personCatalog.memory.aliasesTitle')}
      </p>
      <ul className="flex flex-wrap gap-1.5">
        {person.aliases.map((alias) => (
          <li
            key={alias}
            className="inline-flex items-center gap-1 rounded-full border border-accent-rim bg-accent-soft py-0.5 pl-2.5 pr-1 text-caption font-medium text-accent-text"
          >
            {alias}
            <button
              type="button"
              aria-label={`${t('settings.personCatalog.memory.removeAliasLabel')}: ${alias}`}
              onClick={() => void remove(alias)}
              className="rounded-full p-0.5 text-accent-text/70 transition hover:bg-accent/20 hover:text-accent-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <X size={12} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

function FactRow({
  fact,
  onSave,
  onDelete,
}: {
  fact: PersonFact
  onSave: (text: string) => void
  onDelete: () => void
}) {
  const { t } = useI18n()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(fact.text)
  if (editing) {
    return (
      <li className="flex items-center gap-2 rounded-lg border border-accent-rim bg-surface-raise px-2.5 py-1.5">
        <div className="min-w-0 flex-1">
          <AuroraField
            label={t('settings.personCatalog.memory.editLabel')}
            hideLabel
            value={draft}
            onChange={setDraft}
            onSubmit={() => {
              if (!draft.trim()) return
              onSave(draft.trim())
              setEditing(false)
            }}
          />
        </div>
        <button
          type="button"
          aria-label={t('settings.personCatalog.memory.saveButton')}
          disabled={!draft.trim()}
          onClick={() => {
            onSave(draft.trim())
            setEditing(false)
          }}
          className="rounded-md p-1 text-ok transition hover:bg-ok-soft disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <Check size={14} aria-hidden="true" />
        </button>
        <button
          type="button"
          aria-label={t('settings.personCatalog.memory.cancelButton')}
          onClick={() => setEditing(false)}
          className="rounded-md p-1 text-text-muted transition hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <X size={14} aria-hidden="true" />
        </button>
      </li>
    )
  }
  return (
    <li className="group flex items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-2.5 py-1.5">
      <span className="min-w-0 flex-1 text-caption text-text-main">{fact.text}</span>
      <button
        type="button"
        aria-label={t('settings.personCatalog.memory.editLabel')}
        onClick={() => {
          setDraft(fact.text)
          setEditing(true)
        }}
        className="rounded-md p-1 text-text-subtle transition hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <Pencil size={13} aria-hidden="true" />
      </button>
      <ConfirmButton ariaLabel={t('settings.personCatalog.memory.deleteLabel')} icon={Trash2} onConfirm={onDelete} />
    </li>
  )
}

function MergeRow({ person, rpc, onChanged }: { person: FullPerson; rpc: Rpc; onChanged: () => void }) {
  const { t } = useI18n()
  const [others, setOthers] = useState<Array<{ id: string; primaryName: string; relationToUser?: string }>>([])
  const [sourceId, setSourceId] = useState('')

  useEffect(() => {
    void rpc
      .call<{ success: boolean; data?: Array<{ id: string; primaryName: string; relationToUser?: string }> }>(
        'person_catalog_list',
        { limit: 300, sortBy: 'lastSeenAt' },
      )
      .then((res) => setOthers((res?.data ?? []).filter((p) => p.id !== person.id)))
      .catch(() => setOthers([]))
  }, [rpc, person.id])

  if (!others.length) return null
  const merge = async () => {
    const ok = await runRpc(rpc, 'person_catalog_merge', { targetId: person.id, sourceId, confirm: true }, t)
    if (!ok) return
    setSourceId('')
    useUIStore.getState().addNotification('success', t('settings.personCatalog.memory.mergeDone'))
    onChanged()
  }
  return (
    <div className="border-t border-border-subtle pt-3">
      <p className="mb-2 text-caption text-text-muted">{t('settings.personCatalog.memory.mergeDescription')}</p>
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-48 flex-1">
          <GlassSelect
            label={t('settings.personCatalog.memory.mergeTitle')}
            hideLabel
            searchable
            value={sourceId}
            onChange={setSourceId}
            options={[
              { value: '', label: t('settings.personCatalog.memory.mergePlaceholder') },
              ...others.map((p) => ({
                value: p.id,
                label: p.relationToUser ? `${p.primaryName} · ${p.relationToUser}` : p.primaryName,
              })),
            ]}
          />
        </div>
        <button
          type="button"
          disabled={!sourceId}
          onClick={() => void merge()}
          className="inline-flex items-center gap-1.5 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption font-bold text-warn transition hover:bg-warn/20 disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <Merge size={13} aria-hidden="true" />
          {t('settings.personCatalog.memory.mergeButton')}
        </button>
      </div>
    </div>
  )
}

/** People memory for one person: alias chips, the facts Mika keeps (edit/delete), and merge. */
export function PersonMemorySection({
  person,
  rpc,
  onChanged,
}: {
  person: FullPerson
  rpc: Rpc
  onChanged: () => void
}) {
  const { t } = useI18n()
  const facts = person.facts ?? []
  const editFact = async (factId: string, text?: string) => {
    if (await runRpc(rpc, 'person_catalog_fact_edit', { personId: person.id, factId, text }, t)) onChanged()
  }
  return (
    <GlassSection
      title={t('settings.personCatalog.memory.title')}
      icon={Brain}
      badge={String(facts.length)}
      description={t('settings.personCatalog.memory.description')}
    >
      <div className="space-y-4">
        {person.unnamed && <StatusPill variant="pending" label={t('settings.personCatalog.memory.unnamedBadge')} />}
        <AliasChips person={person} rpc={rpc} onChanged={onChanged} />
        {facts.length ? (
          <ul className="space-y-1.5">
            {facts.map((fact) => (
              <FactRow
                key={`${fact.id}:${fact.text}`}
                fact={fact}
                onSave={(text) => void editFact(fact.id, text)}
                onDelete={() => void editFact(fact.id)}
              />
            ))}
          </ul>
        ) : (
          <p className="text-caption text-text-subtle">{t('settings.personCatalog.memory.empty')}</p>
        )}
        <MergeRow person={person} rpc={rpc} onChanged={onChanged} />
      </div>
    </GlassSection>
  )
}
