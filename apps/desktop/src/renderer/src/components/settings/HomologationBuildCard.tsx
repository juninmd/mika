import { formatLocalDuration } from '@mika/sdk-ui/settings'
import { Terminal } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { GlassCard, SectionLabel } from './ds'

export type BuildIdentity = {
  version: string
  packaged: boolean
  status: string
  uptime: number
  features: number
  tools: number
}

export function HomologationBuildCard({ build }: { build: BuildIdentity | null }) {
  const { t, locale } = useI18n()
  const bootStatusLabel: Record<string, string> = {
    degraded: t('settings.homologation.buildCard.bootStatus.degraded'),
    initializing: t('settings.homologation.buildCard.bootStatus.initializing'),
    ok: t('settings.homologation.buildCard.bootStatus.ok'),
    starting: t('settings.homologation.buildCard.bootStatus.starting'),
  }
  return (
    <GlassCard span={2}>
      <SectionLabel icon={Terminal} className="mb-4">
        {t('settings.homologation.buildCard.title')}
      </SectionLabel>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="space-y-1">
          <span className="text-micro font-semibold uppercase tracking-[0.12em] text-text-subtle">
            {t('settings.homologation.buildCard.buildLabel')}
          </span>
          <p className="text-xs font-medium text-text-main">
            {build
              ? `${build.packaged ? t('settings.homologation.buildCard.packaged') : t('settings.homologation.buildCard.development')} · ${build.version === 'desconhecida' ? t('settings.homologation.buildCard.unknownVersion') : `v${build.version}`}`
              : t('settings.homologation.buildCard.querying')}
          </p>
        </div>
        <div className="space-y-1">
          <span className="text-micro font-semibold uppercase tracking-[0.12em] text-text-subtle">
            {t('settings.homologation.buildCard.bootLabel')}
          </span>
          <p className="text-xs font-medium text-text-main">
            {build
              ? `${bootStatusLabel[build.status] ?? build.status} · ${t('settings.homologation.buildCard.uptime').replace('{{duration}}', formatLocalDuration(build.uptime, locale))}`
              : t('settings.homologation.buildCard.querying')}
          </p>
        </div>
        <div className="space-y-1">
          <span className="text-micro font-semibold uppercase tracking-[0.12em] text-text-subtle">
            {t('settings.homologation.buildCard.runtimeLabel')}
          </span>
          <p className="text-xs font-medium text-text-main">
            {build
              ? `${t('settings.homologation.buildCard.featuresCount').replace('{{count}}', String(build.features))} · ${t('settings.homologation.buildCard.toolsRegistered').replace('{{count}}', String(build.tools))}`
              : t('settings.homologation.buildCard.querying')}
          </p>
        </div>
      </div>
    </GlassCard>
  )
}
