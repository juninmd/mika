import { AuroraField, EmptyState, GlassCard, LazyList, SectionLabel } from '@mika/sdk-ui/settings'
import { Check, Lightbulb, Loader2, Pencil, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { ConfirmButton } from '../../src/components/settings/RichSettingBlocks'
import { truncate } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import type { LearnedFact } from './memory-types'

type FactRowProps = {
  fact: LearnedFact
  onDelete: (id: string) => void
  onUpdate: (id: string, text: string) => Promise<boolean>
  writeBlocked: boolean
}

function FactEditor({
  fact,
  onDone,
  onUpdate,
}: { fact: LearnedFact; onDone: () => void } & Pick<FactRowProps, 'onUpdate'>) {
  const { t } = useI18n()
  const [draft, setDraft] = useState(fact.fact)
  const [saving, setSaving] = useState(false)
  const save = async () => {
    const text = draft.trim()
    if (!text || saving) return
    if (text === fact.fact) return onDone()
    setSaving(true)
    const ok = await onUpdate(fact.id, text)
    setSaving(false)
    if (ok) onDone()
  }
  return (
    <div className="flex items-center gap-2 rounded-lg border border-accent-rim bg-surface-raise px-3 py-2">
      <div className="min-w-0 flex-1">
        <AuroraField
          label={t('settings.memory.panel.editFactLabel')}
          hideLabel
          value={draft}
          onChange={setDraft}
          onSubmit={() => void save()}
        />
      </div>
      <button
        type="button"
        onClick={() => void save()}
        disabled={saving || !draft.trim()}
        aria-label={t('settings.memory.panel.saveFactAriaLabel')}
        className={ICON_BUTTON}
      >
        {saving ? (
          <Loader2 size={11} className="animate-spin" aria-hidden="true" />
        ) : (
          <Check size={11} aria-hidden="true" />
        )}
      </button>
      <button
        type="button"
        onClick={onDone}
        disabled={saving}
        aria-label={t('settings.memory.panel.cancelEditAriaLabel')}
        className={ICON_BUTTON}
      >
        <X size={11} aria-hidden="true" />
      </button>
    </div>
  )
}

const ICON_BUTTON =
  'inline-flex min-h-8 shrink-0 items-center rounded border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption text-text-muted transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50'

export function FactRow({ fact, onDelete, onUpdate, writeBlocked }: FactRowProps) {
  const { t } = useI18n()
  const [editing, setEditing] = useState(false)
  if (editing && !writeBlocked) return <FactEditor fact={fact} onUpdate={onUpdate} onDone={() => setEditing(false)} />
  return (
    <div className="flex items-center gap-3 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2">
      <p className="min-w-0 flex-1 text-caption leading-relaxed text-text-main">{fact.fact}</p>
      <span className="shrink-0 font-mono text-caption text-text-subtle">{Math.round(fact.confidence * 100)}%</span>
      <button
        type="button"
        onClick={() => setEditing(true)}
        disabled={writeBlocked}
        aria-label={`${t('settings.memory.panel.editFactAriaPrefix')} ${truncate(fact.fact, 40)}`}
        className={ICON_BUTTON}
      >
        <Pencil size={11} aria-hidden="true" />
      </button>
      <ConfirmButton
        icon={Trash2}
        ariaLabel={`${t('settings.memory.panel.deleteFactAriaPrefix')} ${truncate(fact.fact, 40)}`}
        confirmLabel={t('settings.memory.panel.deleteButton')}
        onConfirm={() => onDelete(fact.id)}
      />
    </div>
  )
}

export function FactsCard({
  facts,
  filter,
  onFilterChange,
  newFact,
  onNewFactChange,
  onAdd,
  onDelete,
  onUpdate,
  writeBlocked,
  busy,
}: {
  facts: LearnedFact[]
  filter: string
  onFilterChange: (q: string) => void
  newFact: string
  onNewFactChange: (t: string) => void
  onAdd: () => void
  onDelete: (id: string) => void
  onUpdate: (id: string, text: string) => Promise<boolean>
  writeBlocked: boolean
  busy: boolean
}) {
  const { t } = useI18n()
  const filtered = facts.filter((f) => f.fact.toLowerCase().includes(filter.toLowerCase()))
  const list = filtered.length === 0 ? null : filtered
  return (
    <GlassCard>
      <SectionLabel icon={Lightbulb} count={filtered.length}>
        {t('settings.memory.panel.factsTitle')}
      </SectionLabel>
      <div className="flex flex-col gap-2 sm:flex-row">
        <div className="min-w-0 flex-1">
          <AuroraField
            label={t('settings.memory.panel.factsTitle')}
            hideLabel
            value={newFact}
            onChange={onNewFactChange}
            onSubmit={() => !busy && !writeBlocked && onAdd()}
            placeholder={t('settings.memory.panel.factsAddPlaceholder')}
          />
        </div>
        <button
          type="button"
          onClick={() => void onAdd()}
          disabled={busy || writeBlocked}
          className="rounded-lg border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-semibold text-accent hover:bg-accent/25 disabled:cursor-not-allowed disabled:bg-surface-raise-strong disabled:text-text-muted"
        >
          {busy ? <Loader2 size={12} className="animate-spin" /> : t('settings.memory.panel.addButton')}
        </button>
      </div>
      <div className="mt-2">
        <AuroraField
          label={t('settings.memory.panel.factsFilterAriaLabel')}
          hideLabel
          type="search"
          value={filter}
          onChange={onFilterChange}
          placeholder={t('settings.memory.panel.factsFilterPlaceholder')}
        />
      </div>
      <div className="mt-3 space-y-1.5">
        {!list && (
          <EmptyState
            icon={Lightbulb}
            title={
              facts.length > 0
                ? `${t('settings.memory.panel.factsEmptyFiltered')} “${filter}”.`
                : t('settings.memory.panel.factsEmptyDefault')
            }
          />
        )}
        {list && (
          <LazyList
            items={list}
            itemKey={(f) => f.id}
            estimatedItemHeight={38}
            className="space-y-1.5"
            renderItem={(f) => <FactRow fact={f} onDelete={onDelete} onUpdate={onUpdate} writeBlocked={writeBlocked} />}
          />
        )}
      </div>
    </GlassCard>
  )
}
