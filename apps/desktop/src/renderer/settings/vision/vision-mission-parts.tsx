import type { ComponentType } from 'react'
import type { Translate } from '../../src/hooks/useI18n'

export function formatInterval(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(value % 1000 === 0 ? 0 : 1)}s`
  return `${value}ms`
}

type PillState = 'active' | 'blocked' | 'paused'

function pillState(on: boolean, blocked: boolean): PillState {
  if (!on) return 'paused'
  return blocked ? 'blocked' : 'active'
}

const PILL_LABEL = {
  active: 'settings.vision.mission.status.active',
  blocked: 'settings.vision.mission.status.blocked',
  paused: 'settings.vision.mission.status.paused',
} as const

const PILL_CLASS: Record<PillState, string> = {
  active: 'border-ok-rim bg-ok-soft text-ok',
  blocked: 'border-warn-rim bg-warn-soft text-warn',
  paused: 'border-border-subtle bg-surface-raise text-text-muted',
}

export function ReadinessPill({
  label,
  on,
  blocked,
  icon: Icon,
  t,
}: {
  label: string
  on: boolean
  blocked: boolean
  icon: ComponentType<{ size?: number; className?: string }>
  t: Translate
}) {
  const state = pillState(on, blocked)
  return (
    <div
      className={`flex items-center justify-between gap-2 rounded-md border px-2.5 py-2 ${PILL_CLASS[state]}`}
      title={state === 'blocked' ? t('settings.vision.mission.status.blockedHint') : undefined}
    >
      <span className="flex min-w-0 items-center gap-1.5 text-caption font-semibold">
        <Icon size={12} className="shrink-0" />
        <span className="truncate">{label}</span>
      </span>
      <span className="text-micro font-bold uppercase tracking-[0.12em]">{t(PILL_LABEL[state])}</span>
    </div>
  )
}

export function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border-subtle bg-surface-raise px-2.5 py-2">
      <p className="text-micro font-black uppercase tracking-[0.12em] text-text-subtle">{label}</p>
      <p className="mt-1 line-clamp-2 break-words text-caption font-semibold leading-snug text-text-main" title={value}>
        {value}
      </p>
    </div>
  )
}
