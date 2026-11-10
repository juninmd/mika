import type { StatTone } from '@mika/sdk-ui/settings'
import { CheckCircle2, CircleAlert, CircleDashed } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { GlassBox } from './ds'
import { TagList } from './TagList'

const toneClasses: Record<StatTone, string> = {
  info: 'border-info-rim bg-info-soft text-info',
  ok: 'border-ok-rim bg-ok-soft text-ok',
  warn: 'border-warn-rim bg-warn-soft text-warn',
  danger: 'border-danger-rim bg-danger-soft text-danger',
  accent: 'border-accent-rim bg-accent-soft text-accent',
  neutral: 'border-border-subtle bg-surface-raise text-text-main',
}

export function SettingsRiskBanner({
  title,
  detail,
  tone = 'warn',
}: {
  title: string
  detail: string
  tone?: StatTone
}) {
  const Icon = tone === 'danger' || tone === 'warn' ? CircleAlert : CheckCircle2
  return (
    <div
      role={tone === 'danger' || tone === 'warn' ? 'alert' : 'status'}
      className={`flex gap-3 rounded-xl border px-4 py-3 ${toneClasses[tone]}`}
    >
      <Icon size={16} className="mt-0.5 shrink-0" aria-hidden="true" />
      <div>
        <p className="text-[12px] font-semibold text-text-main">{title}</p>
        <p className="mt-1 text-caption leading-relaxed text-text-muted">{detail}</p>
      </div>
    </div>
  )
}

export function SettingsChecklist({
  title,
  items,
}: {
  title: string
  items: Array<{ label: string; ok: boolean; detail?: string }>
}) {
  return (
    <GlassBox variant="card">
      <p className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{title}</p>
      <ul className="mt-3 grid gap-x-6 gap-y-2 sm:grid-cols-2 xl:grid-cols-3">
        {items.map((item) => {
          const Icon = item.ok ? CheckCircle2 : CircleDashed
          return (
            <li key={item.label} className="flex items-start gap-2">
              <Icon size={14} className={item.ok ? 'mt-0.5 text-ok' : 'mt-0.5 text-text-subtle'} aria-hidden="true" />
              <div className="min-w-0">
                <p className="text-[12px] font-medium text-text-main">{item.label}</p>
                {item.detail ? <p className="text-caption leading-snug text-text-muted">{item.detail}</p> : null}
              </div>
            </li>
          )
        })}
      </ul>
    </GlassBox>
  )
}

export function SettingsTagInput({
  label,
  tags,
  onChange,
  placeholder = 'Adicionar item…',
}: {
  label: string
  tags: string[]
  onChange: (tags: string[]) => void
  placeholder?: string
}) {
  return (
    <TagList
      label={label}
      tags={tags}
      onChange={onChange}
      placeholder={placeholder}
      addLabel="Adicionar"
      removeLabel="Remover"
    />
  )
}

export { ConfirmButton, type ConfirmButtonProps } from '@mika/sdk-ui/settings'

export function SettingsFeatureList({
  title,
  items,
}: {
  title: string
  items: Array<{ label: string; description?: string; enabled: boolean }>
}) {
  const { t } = useI18n()
  return (
    <GlassBox variant="card">
      <p className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{title}</p>
      {/* A grid of short rows: one full-width bar per two-word label left most of each row empty. */}
      <ul className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {items.map((item) => (
          <li key={item.label} className="flex items-start gap-2.5 rounded-lg border border-border-subtle px-3 py-2">
            <span
              role="img"
              className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${item.enabled ? 'bg-ok' : 'border border-text-muted'}`}
              aria-label={t(item.enabled ? 'status.enabled' : 'status.disabled')}
            />
            <div className="min-w-0">
              <p className={`text-xs font-medium ${item.enabled ? 'text-text-main' : 'text-text-muted'}`}>
                {item.label}
              </p>
              {item.description && <p className="text-caption leading-snug text-text-muted">{item.description}</p>}
            </div>
          </li>
        ))}
      </ul>
    </GlassBox>
  )
}
