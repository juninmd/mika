import { GlassCard } from '@mika/sdk-ui/settings'
import type { LucideIcon } from 'lucide-react'
import { CheckCircle2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import type { ProfileCompletion, Props } from './user-profile-config'

type ProfileSectionProps = {
  title: string
  description: string
  icon: LucideIcon
  children: ReactNode
}

export function ProfileSection({ title, description, icon: Icon, children }: ProfileSectionProps) {
  return (
    <GlassCard span="full">
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent">
            <Icon size={17} aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-black uppercase tracking-[0.12em] text-text-main">{title}</h3>
            <p className="mt-1 max-w-2xl text-xs leading-relaxed text-text-muted">{description}</p>
          </div>
        </div>
      </div>
      {children}
    </GlassCard>
  )
}

function ProfileFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2">
      <p className="text-micro font-bold uppercase tracking-[0.14em] text-text-muted">{label}</p>
      <p className="mt-1 line-clamp-2 break-words text-xs font-semibold leading-snug text-text-main" title={value}>
        {value}
      </p>
    </div>
  )
}

export function ProfileSnapshot({ props, completion }: { props: Props; completion: ProfileCompletion }) {
  const { t } = useI18n()
  const displayName = props.nickname || props.name || t('settings.user.panel.defaultName')
  const location =
    [props.city, props.state, props.country].filter(Boolean).join(', ') || t('settings.user.panel.locationUnset')
  const work = [props.role, props.profession].filter(Boolean).join(' / ') || t('settings.user.panel.workUnset')
  const partner = props.partnerName
    ? `${props.partnerName}${props.partnerRelationship ? ` (${props.partnerRelationship})` : ''}`
    : t('settings.user.panel.partnerUnset')

  return (
    <GlassCard span="full">
      <div className="grid gap-5 lg:grid-cols-[minmax(0,1.2fr)_minmax(260px,0.8fr)]">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-accent-rim bg-accent-soft text-xl font-black uppercase text-accent">
              {displayName.slice(0, 2)}
            </div>
            <div className="min-w-0">
              <p className="text-micro font-black uppercase tracking-[0.18em] text-accent-text">
                {t('settings.user.panel.badge')}
              </p>
              <h2 className="mt-1 truncate text-2xl font-black tracking-tight text-text-main">{displayName}</h2>
              <p className="mt-1 text-xs leading-relaxed text-text-muted">
                {t('settings.user.panel.snapshotDescription')}
              </p>
            </div>
          </div>

          <div className="mt-5 grid gap-2 sm:grid-cols-3">
            <ProfileFact label={t('settings.user.panel.factLocation')} value={location} />
            <ProfileFact label={t('settings.user.panel.factWork')} value={work} />
            <ProfileFact label={t('settings.user.panel.factRelationship')} value={partner} />
          </div>
        </div>

        <div role="status" className="rounded-2xl border border-border-subtle bg-surface-raise/40 p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-micro font-black uppercase tracking-[0.18em] text-text-muted">
                {t('settings.user.panel.completion')}
              </p>
              <p className="mt-1 text-3xl font-black text-text-main">{completion.percent}%</p>
            </div>
            {completion.filled === completion.total ? (
              <CheckCircle2 className="text-ok" size={28} aria-hidden="true" />
            ) : (
              <p className="shrink-0 text-right text-caption font-semibold text-text-muted">
                {`${completion.filled} ${t('settings.user.panel.completionOf')} ${completion.total} ${t('settings.user.panel.completionFields')}`}
              </p>
            )}
          </div>
          <div className="mt-4 h-2 overflow-hidden rounded-full bg-surface-raise">
            <div
              className="h-full w-full origin-left rounded-full bg-info transition-transform duration-500 ease-out"
              style={{ transform: `scaleX(${completion.percent / 100})` }}
            />
          </div>
          <p className="mt-3 text-xs leading-relaxed text-text-muted">{t('settings.user.panel.completionHint')}</p>
        </div>
      </div>
    </GlassCard>
  )
}
