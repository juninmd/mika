import { Eye, Gamepad2, Hash, Monitor, Radio, ShieldAlert, Smile } from 'lucide-react'
import type { ComponentType } from 'react'
import { compactText } from '../../src/components/settings/settings-utils'
import type { Translate } from '../../src/hooks/useI18n'
import { type Observation, relativeTime } from './vision-monitor-model'

export function VisionSignalGrid({ observation, t }: { observation: Observation; t: Translate }) {
  return (
    <div className="grid grid-cols-2 gap-2">
      <SignalTile
        icon={Monitor}
        label={t('settings.vision.monitor.signal.app.label')}
        value={
          observation.foregroundApp?.title ??
          observation.foregroundApp?.processName ??
          t('settings.vision.monitor.signal.app.unknown')
        }
        active={Boolean(observation.foregroundApp?.title || observation.foregroundApp?.processName)}
      />
      <SignalTile
        icon={Gamepad2}
        label={t('settings.vision.monitor.signal.game.label')}
        value={
          observation.isLikelyGame
            ? t('settings.vision.monitor.signal.game.detected')
            : t('settings.vision.monitor.signal.game.low')
        }
        active={Boolean(observation.isLikelyGame)}
      />
      <SignalTile
        icon={Smile}
        label={t('settings.vision.monitor.signal.mood.label')}
        value={
          observation.isFunny
            ? t('settings.vision.monitor.signal.mood.funny')
            : t('settings.vision.monitor.signal.mood.neutral')
        }
        active={Boolean(observation.isFunny)}
        variant={observation.isFunny ? 'fuchsia' : undefined}
      />
      <SignalTile
        icon={Hash}
        label={t('settings.vision.monitor.signal.meme.label')}
        value={observation.memeContext ?? t('settings.vision.monitor.signal.meme.none')}
        active={Boolean(observation.memeContext)}
      />
    </div>
  )
}

export function ObservationHistory({ observations, t }: { observations: Observation[]; t: Translate }) {
  if (observations.length === 0) return null

  return (
    <div className="space-y-1.5">
      <p className="text-micro font-bold uppercase tracking-widest text-text-subtle">
        {t('settings.vision.monitor.historyTitle')}
      </p>
      {observations.map((obs) => (
        <div
          key={obs.id}
          className="flex items-start gap-2 rounded-lg px-2.5 py-2"
          style={{ background: 'var(--surface-raise)' }}
        >
          <Eye size={10} className="mt-0.5 shrink-0 text-text-subtle" />
          <p className="flex-1 text-caption leading-relaxed text-text-subtle">{compactText(obs.analysis, 120)}</p>
          <span className="shrink-0 font-mono text-caption text-text-subtle">{relativeTime(obs.at, t)}</span>
        </div>
      ))}
    </div>
  )
}

export function EmptyObservation({ t, blockedReason }: { t: Translate; blockedReason?: string }) {
  if (blockedReason) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-warn-rim bg-warn-soft px-4 py-8 text-center">
        <ShieldAlert size={16} className="text-warn" aria-hidden="true" />
        <span className="text-caption font-semibold text-warn">{t('settings.vision.monitor.blocked')}</span>
        <span className="max-w-md text-caption text-text-muted">{blockedReason}</span>
      </div>
    )
  }
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border-subtle py-8 text-center">
      <Radio size={16} className="text-text-muted animate-pulse" />
      <span className="text-caption font-medium tracking-wide text-text-muted">
        {t('settings.vision.monitor.empty')}
      </span>
    </div>
  )
}

function SignalTile({
  icon: Icon,
  label,
  value,
  active,
  variant,
}: {
  icon: ComponentType<{ size?: number; className?: string }>
  label: string
  value: string
  active: boolean
  variant?: 'fuchsia'
}) {
  const isFuchsia = variant === 'fuchsia'
  const activeClass = isFuchsia
    ? 'border-accent-rim bg-accent-soft text-accent'
    : 'border-info-rim bg-info-soft text-info'

  return (
    <div
      className={`rounded-lg border px-2.5 py-2 ${
        active ? activeClass : 'border-border-subtle bg-surface-raise text-text-muted'
      }`}
    >
      <div className="flex items-center gap-1.5">
        <Icon size={11} className="shrink-0" />
        <p className="text-micro font-black uppercase tracking-[0.12em]">{label}</p>
      </div>
      <p className="mt-1 truncate font-mono text-caption">{value}</p>
    </div>
  )
}
