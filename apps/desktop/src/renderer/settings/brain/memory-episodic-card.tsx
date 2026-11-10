import { formatRelativeTime } from '@mika/sdk/utils/time-format'
import { AuroraField, EmptyState, GlassCard, SectionLabel } from '@mika/sdk-ui/settings'
import { History, Loader2, RefreshCw, Search, Trash2 } from 'lucide-react'
import { ConfirmButton } from '../../src/components/settings/RichSettingBlocks'
import { truncate } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'

import type { EpisodicMemory } from './memory-types'

export function ErrorNote({ message }: { message: string }) {
  return (
    <p className="rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-caption text-danger">{message}</p>
  )
}

export function EpisodicCard({
  memories,
  query,
  searchedTerm,
  onQueryChange,
  onSearch,
  onDelete,
  busy,
}: {
  memories: EpisodicMemory[]
  query: string
  searchedTerm: string | null
  onQueryChange: (q: string) => void
  onSearch: (query?: string) => void
  onDelete: (id: string) => void
  busy?: boolean
}) {
  const { t } = useI18n()
  const list = memories.length === 0 ? null : memories
  return (
    <GlassCard>
      <SectionLabel icon={History} count={memories.length}>
        {t('settings.memory.panel.episodicTitle')}
      </SectionLabel>
      <div className="flex flex-col gap-2 sm:flex-row">
        <div className="min-w-0 flex-1">
          <AuroraField
            label={t('settings.memory.panel.episodicSearchAriaLabel')}
            hideLabel
            type="search"
            value={query}
            onChange={onQueryChange}
            onSubmit={() => !busy && onSearch(query.trim() || undefined)}
            placeholder={t('settings.memory.panel.episodicSearchPlaceholder')}
          />
        </div>
        <button
          type="button"
          onClick={() => void onSearch(query.trim() || undefined)}
          disabled={busy}
          className="inline-flex items-center justify-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-semibold text-accent hover:bg-accent/25 disabled:cursor-not-allowed disabled:bg-surface-raise-strong disabled:text-text-muted"
        >
          {busy ? <Loader2 size={11} className="animate-spin" /> : <Search size={11} />}
          {t('button.search')}
        </button>
        <button
          type="button"
          onClick={() => void onSearch()}
          disabled={busy}
          className="inline-flex items-center justify-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-caption font-semibold text-text-main hover:text-ok disabled:opacity-50"
        >
          <RefreshCw size={11} />
          {t('settings.memory.panel.recentButton')}
        </button>
      </div>
      <div className="mt-3 space-y-1.5">
        {!list && (
          <EmptyState
            icon={History}
            title={
              searchedTerm
                ? `${t('settings.memory.panel.episodicEmptyForTerm')} “${searchedTerm}”.`
                : t('settings.memory.panel.episodicEmptyDefault')
            }
          />
        )}
        {list?.map((m) => (
          <div
            key={m.id}
            className="flex items-start gap-3 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2"
          >
            <p className="min-w-0 flex-1 truncate text-caption text-text-main" title={m.content}>
              {truncate(m.content, 120)}
            </p>
            <span className="shrink-0 text-caption text-text-subtle">{formatRelativeTime(m.timestamp, t)}</span>
            <ConfirmButton
              icon={Trash2}
              ariaLabel={`${t('settings.memory.panel.deleteMemoryAriaPrefix')} ${truncate(m.content, 40)}`}
              confirmLabel={t('settings.memory.panel.deleteButton')}
              onConfirm={() => onDelete(m.id)}
            />
          </div>
        ))}
      </div>
    </GlassCard>
  )
}
