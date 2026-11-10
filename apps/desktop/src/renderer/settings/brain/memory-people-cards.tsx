import { AuroraField, EmptyState, GlassCard, LazyList, SectionLabel } from '@mika/sdk-ui/settings'
import { HeartHandshake, IdCard, Loader2, Trash2, Users } from 'lucide-react'
import { ConfirmButton } from '../../src/components/settings/RichSettingBlocks'
import { useI18n } from '../../src/hooks/useI18n'
import type { PersonRecord, RelationshipEdge } from './memory-types'

export function PeopleCard({
  people,
  onForget,
  loading,
  error,
}: {
  people: PersonRecord[]
  onForget: (name: string) => void
  loading?: boolean
  error?: string | null
}) {
  const { t } = useI18n()
  const list = people.length === 0 ? null : people
  return (
    <GlassCard>
      <SectionLabel icon={Users} count={people.length}>
        {t('settings.memory.panel.peopleTitle')}
      </SectionLabel>
      <div className="space-y-1.5">
        {!list && !loading && !error && <EmptyState icon={Users} title={t('settings.memory.panel.peopleEmpty')} />}
        {list && (
          <LazyList
            items={list}
            itemKey={(p) => p.id}
            estimatedItemHeight={46}
            className="space-y-1.5"
            renderItem={(p) => (
              <div className="flex items-center gap-3 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-caption font-semibold text-text-main">{p.primaryName}</p>
                  <p className="truncate text-caption text-text-subtle">
                    {p.type}
                    {p.aliases.length > 0 ? ` · ${p.aliases.slice(0, 2).join(', ')}` : ''}
                  </p>
                </div>
                <ConfirmButton
                  label={t('settings.memory.panel.forgetButton')}
                  ariaLabel={`${t('settings.memory.panel.forgetButton')} ${p.primaryName}`}
                  confirmLabel={t('settings.memory.panel.forgetButton')}
                  onConfirm={() => onForget(p.primaryName)}
                />
              </div>
            )}
          />
        )}
      </div>
    </GlassCard>
  )
}

export function RelationsCard({
  relationships,
  onDelete,
  loading,
  error,
}: {
  relationships: RelationshipEdge[]
  onDelete: (id: string) => void
  loading?: boolean
  error?: string | null
}) {
  const { t } = useI18n()
  const list = relationships.length === 0 ? null : relationships
  return (
    <GlassCard>
      <SectionLabel icon={HeartHandshake} count={relationships.length}>
        {t('settings.memory.panel.relationsTitle')}
      </SectionLabel>
      <div className="space-y-1.5">
        {!list && !loading && !error && (
          <EmptyState icon={HeartHandshake} title={t('settings.memory.panel.relationsEmpty')} />
        )}
        {list && (
          <LazyList
            items={list}
            itemKey={(r) => r.id}
            estimatedItemHeight={40}
            className="space-y-1.5"
            renderItem={(r) => (
              <div className="flex items-center gap-3 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2">
                <p className="min-w-0 flex-1 truncate text-caption text-text-main">
                  <span className="text-accent">{r.label}</span>
                  <span className="text-text-subtle"> · {r.personId.replace(/-/g, ' ')}</span>
                </p>
                <ConfirmButton
                  icon={Trash2}
                  ariaLabel={`${t('settings.memory.panel.deleteRelationAriaPrefix')} ${r.label}`}
                  confirmLabel={t('settings.memory.panel.deleteButton')}
                  onConfirm={() => onDelete(r.id)}
                />
              </div>
            )}
          />
        )}
      </div>
    </GlassCard>
  )
}

export function ProfileCard({
  name,
  onNameChange,
  onSave,
  onForget,
  busy,
}: {
  name: string
  onNameChange: (n: string) => void
  onSave: () => void
  onForget: () => void
  busy: boolean
}) {
  const { t } = useI18n()
  return (
    <GlassCard>
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
        <div className="min-w-0 flex-1">
          <SectionLabel icon={IdCard}>{t('settings.memory.panel.profileNameLabel')}</SectionLabel>
          <AuroraField
            label={t('settings.memory.panel.profileNameLabel')}
            hideLabel
            value={name}
            onChange={onNameChange}
            onSubmit={() => !busy && onSave()}
            placeholder={t('settings.memory.panel.profileNamePlaceholder')}
          />
        </div>
        <button
          type="button"
          onClick={() => void onSave()}
          disabled={busy}
          className="rounded-lg border border-accent-rim bg-accent-soft px-4 py-2 text-xs font-semibold text-accent hover:bg-accent/25 disabled:cursor-not-allowed disabled:bg-surface-raise-strong disabled:text-text-muted"
        >
          {busy ? <Loader2 size={12} className="animate-spin" /> : t('button.save')}
        </button>
        <div className="flex items-end sm:ml-2 sm:border-l sm:border-border-subtle sm:pl-3">
          <ConfirmButton
            label={t('settings.memory.panel.forgetButton')}
            ariaLabel={t('settings.memory.panel.forgetProfileAriaLabel')}
            confirmLabel={t('settings.memory.panel.forgetButton')}
            onConfirm={onForget}
            disabled={busy}
          />
        </div>
      </div>
    </GlassCard>
  )
}
