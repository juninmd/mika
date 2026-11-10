import { AuroraField, EmptyState, GlassCard, LazyList, SectionLabel } from '@mika/sdk-ui/settings'
import { Lightbulb, Loader2, Trash2 } from 'lucide-react'
import { ConfirmButton } from '../../src/components/settings/RichSettingBlocks'
import { truncate } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import type { LearnedFact } from './memory-types'

export function FactRow({ fact, onDelete }: { fact: LearnedFact; onDelete: (id: string) => void }) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2">
      <p className="min-w-0 flex-1 text-caption leading-relaxed text-text-main">{fact.fact}</p>
      <span className="shrink-0 font-mono text-caption text-text-subtle">{Math.round(fact.confidence * 100)}%</span>
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
  busy,
}: {
  facts: LearnedFact[]
  filter: string
  onFilterChange: (q: string) => void
  newFact: string
  onNewFactChange: (t: string) => void
  onAdd: () => void
  onDelete: (id: string) => void
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
            onSubmit={() => !busy && onAdd()}
            placeholder={t('settings.memory.panel.factsAddPlaceholder')}
          />
        </div>
        <button
          type="button"
          onClick={() => void onAdd()}
          disabled={busy}
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
            renderItem={(f) => <FactRow fact={f} onDelete={onDelete} />}
          />
        )}
      </div>
    </GlassCard>
  )
}
