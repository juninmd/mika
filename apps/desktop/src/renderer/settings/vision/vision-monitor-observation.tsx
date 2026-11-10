import { compactText } from '../../src/components/settings/settings-utils'
import type { Translate } from '../../src/hooks/useI18n'
import { type Observation, relativeTime } from './vision-monitor-model'

export function VisionStatusBar({
  active,
  hasObservations,
  totalCount,
  t,
}: {
  active: boolean
  hasObservations: boolean
  totalCount: number
  t: Translate
}) {
  const dotClass = active
    ? 'animate-pulse bg-info shadow-[0_0_10px_var(--info)]'
    : hasObservations
      ? 'bg-surface-raise'
      : 'bg-bg-surface'

  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-2">
        <span className={`h-2 w-2 rounded-full transition duration-500 ${dotClass}`} />
        <span className="text-caption font-semibold uppercase tracking-widest text-text-muted">
          {active
            ? t('settings.vision.monitor.status.observing')
            : hasObservations
              ? t('settings.vision.monitor.status.standby')
              : t('settings.vision.monitor.status.waiting')}
        </span>
      </div>
      {totalCount > 0 && (
        <span className="font-mono text-caption text-text-subtle">
          {totalCount}{' '}
          {totalCount !== 1 ? t('settings.vision.monitor.count.plural') : t('settings.vision.monitor.count.singular')}
        </span>
      )}
    </div>
  )
}

export function LatestObservation({
  observation,
  active,
  t,
}: {
  observation: Observation
  active: boolean
  t: Translate
}) {
  const isFunny = observation.isFunny

  return (
    <div
      className={`rounded-xl border p-3 transition duration-500 ${
        isFunny
          ? 'border-accent/40 bg-accent-soft shadow-[0_0_15px_color-mix(in_srgb,var(--accent)_15%,transparent)]'
          : ''
      }`}
      style={
        isFunny
          ? {}
          : {
              borderColor: active ? 'var(--accent-rim)' : 'var(--border-subtle)',
              background: active ? 'var(--accent-soft)' : 'rgba(0,0,0,0.2)',
            }
      }
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          {observation.isLikelyGame && (
            <span className="rounded-md bg-accent-soft px-1.5 py-0.5 text-micro font-bold uppercase tracking-widest text-accent">
              {t('settings.vision.monitor.badge.game')}
            </span>
          )}
          {observation.foregroundApp?.title && (
            <span className="truncate text-caption font-medium text-text-muted" style={{ color: 'var(--accent)' }}>
              {observation.foregroundApp.title}
            </span>
          )}
        </div>
        <span className="shrink-0 font-mono text-caption text-text-subtle">{relativeTime(observation.at, t)}</span>
      </div>
      <p className="text-caption leading-relaxed text-text-main">{compactText(observation.analysis)}</p>

      {observation.people && observation.people.length > 0 && (
        <div className="mt-2.5 pt-2 border-t border-border-subtle flex flex-wrap gap-1.5">
          {observation.people.map((p) => (
            <span
              key={`${p.name}:${p.type}:${p.confidence ?? ''}`}
              className="inline-flex items-center gap-1 rounded bg-bg-surface/60 px-1.5 py-0.5 text-caption text-text-main border border-border-subtle"
            >
              <span className="font-semibold text-info">{p.name}</span>
              {p.type && (
                <span className="text-micro text-text-muted uppercase tracking-wider">
                  ({p.type.replace('_', ' ')})
                </span>
              )}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
