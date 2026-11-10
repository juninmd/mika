import { GlassCard, StatTile, StatusPill } from '@mika/sdk-ui/settings'
import { Archive, CircleHelp, History, Lightbulb, MessagesSquare, Timer, Users } from 'lucide-react'
import { useI18n } from '../../src/hooks/useI18n'
import type { SnapshotData } from './memory-panel-sections'

type LayersProps = {
  snapshot: SnapshotData | null
  counts: { episodic: number; userFacts: number }
}

function LayerHeader({ icon: Icon, title, description }: { icon: typeof Timer; title: string; description: string }) {
  return (
    <div className="mb-4 flex items-start gap-3">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent">
        <Icon size={17} aria-hidden="true" />
      </div>
      <div>
        <h3 className="text-xs font-bold uppercase tracking-[0.12em] text-text-main">{title}</h3>
        <p className="mt-1 text-caption leading-relaxed text-text-muted">{description}</p>
      </div>
    </div>
  )
}

function ShortTermLayer({ shortTerm }: { shortTerm: SnapshotData['shortTerm'] }) {
  const { t } = useI18n()
  const facts = shortTerm?.sessionFacts ?? []
  return (
    <GlassCard hover={false}>
      <LayerHeader
        icon={Timer}
        title={t('settings.memory.layers.shortTitle')}
        description={t('settings.memory.layers.shortDescription')}
      />
      <StatTile
        icon={MessagesSquare}
        label={t('settings.memory.layers.messages')}
        value={String(shortTerm?.messages ?? 0)}
        className="min-h-0 p-3"
      />
      <p className="mb-1 mt-4 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.memory.layers.summary')}
      </p>
      <p className="text-caption leading-relaxed text-text-main">
        {shortTerm?.rollingSummary || <span className="text-text-subtle">{t('settings.memory.layers.noSummary')}</span>}
      </p>
      <p className="mb-1 mt-4 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.memory.layers.sessionFacts')}
      </p>
      {facts.length ? (
        <ul className="flex flex-wrap gap-1.5">
          {facts.map((fact) => (
            <li key={fact}>
              <StatusPill variant="info" label={fact} />
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-caption text-text-subtle">{t('settings.memory.layers.noSessionFacts')}</p>
      )}
    </GlassCard>
  )
}

function LongTermLayer({ snapshot, counts }: LayersProps) {
  const { t } = useI18n()
  const people = snapshot?.peopleMemory
  return (
    <GlassCard hover={false}>
      <LayerHeader
        icon={Archive}
        title={t('settings.memory.layers.longTitle')}
        description={t('settings.memory.layers.longDescription')}
      />
      <div className="grid grid-cols-2 gap-2">
        <StatTile
          icon={Lightbulb}
          label={t('settings.memory.layers.userFacts')}
          value={String(counts.userFacts)}
          className="min-h-0 p-3"
        />
        <StatTile
          icon={History}
          label={t('settings.memory.layers.episodes')}
          value={String(counts.episodic)}
          className="min-h-0 p-3"
        />
        <StatTile
          icon={Users}
          label={t('settings.memory.layers.people')}
          value={String(people?.people ?? 0)}
          className="min-h-0 p-3"
        />
        <StatTile
          icon={Lightbulb}
          label={t('settings.memory.layers.peopleFacts')}
          value={String(people?.facts ?? 0)}
          className="min-h-0 p-3"
        />
      </div>
      {(people?.pendingClarifications ?? 0) > 0 && (
        <div className="mt-3 flex items-center gap-2">
          <CircleHelp size={14} aria-hidden="true" className="text-warn" />
          <StatusPill
            variant="warn"
            label={`${t('settings.memory.layers.pending')}: ${people?.pendingClarifications}`}
          />
        </div>
      )}
    </GlassCard>
  )
}

/** Short-term vs long-term memory, side by side: what is in the prompt now vs what is retrieved by relevance. */
export function MemoryLayers(props: LayersProps) {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <ShortTermLayer shortTerm={props.snapshot?.shortTerm} />
      <LongTermLayer {...props} />
    </div>
  )
}
