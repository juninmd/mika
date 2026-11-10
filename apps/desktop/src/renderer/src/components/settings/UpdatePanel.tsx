import { getLocale, t } from '@mika/sdk/i18n'
import { clampPercent } from '@mika/sdk/utils/clamp'
import { formatLocalTime } from '@mika/sdk-ui'
import type { PillVariant } from '@mika/sdk-ui/settings'
import { Download, RefreshCw, RotateCw, Store } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useUpdateChecker } from '../../hooks/useUpdateChecker'
import { getElectronApi } from '../../lib/shell-api'
import { AutoUpdateSection } from './AutoUpdateSection'
import { GlassCard, GlassSection, InfoRow, PageHero, StatTile, StatusPill } from './ds'

type UpdateFlags = {
  installing: boolean
  downloaded: boolean
  downloading: boolean
  checking: boolean
  available: boolean
  progress: number
  checked: boolean
  failed: boolean
}

export function statusLabelFor(f: UpdateFlags): string {
  const locale = getLocale()
  if (f.installing) return t('settings.update.status.installing', locale)
  if (f.downloaded) return t('settings.update.status.readyToInstall', locale)
  if (f.downloading) {
    const pct = clampPercent(Math.round(f.progress))
    return `${t('settings.update.status.downloading', locale)}${f.progress ? ` ${pct}%` : ''}...`
  }
  if (f.checking) return t('settings.update.status.checking', locale)
  if (f.available) return t('settings.update.status.available', locale)
  // A failed check still sets `checked`; reporting "Atualizado" next to the error contradicted it.
  if (f.failed) return t('settings.update.status.failed', locale)

  return f.checked ? t('settings.update.status.upToDate', locale) : t('settings.update.status.neverChecked', locale)
}

function statusVariantFor(f: UpdateFlags): PillVariant {
  if (f.installing || f.downloading || f.checking) return 'loading'
  if (f.downloaded || f.available) return 'new'
  if (f.failed) return 'error'
  return f.checked ? 'ok' : 'pending'
}

type BuildIdentity = { version: string; packaged: boolean }

function UpdateFacts({ build, lastCheckAt }: { build: BuildIdentity | null; lastCheckAt: number | null }) {
  const { t, locale } = useI18n()
  return (
    <div className="mb-4 space-y-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-3">
      <InfoRow
        label={t('settings.update.facts.lastCheck')}
        value={lastCheckAt ? formatLocalTime(lastCheckAt, locale) : '—'}
        mono
      />
      <InfoRow label={t('settings.update.facts.buildKind')} value={buildKindLabel(build)} />
    </div>
  )
}

function buildKindLabel(build: BuildIdentity | null): string {
  if (!build) return '—'
  const locale = getLocale()
  return build.packaged ? t('settings.update.facts.packaged', locale) : t('settings.update.facts.development', locale)
}

function UpdateHero({
  flags,
  build,
  version,
}: {
  flags: UpdateFlags
  build: BuildIdentity | null
  version: string | null
}) {
  const { t } = useI18n()
  return (
    <PageHero
      icon={RotateCw}
      title={t('settings.update.title')}
      description={t('settings.update.description')}
      pills={
        <>
          <StatusPill variant={statusVariantFor(flags)} label={statusLabelFor(flags)} dot />
          <StatusPill variant="neutral" label={buildKindLabel(build)} />
        </>
      }
      aside={
        <>
          <StatTile
            label={t('settings.update.facts.installedVersion')}
            value={build ? `v${build.version}` : t('settings.update.facts.checking')}
          />
          <StatTile
            label={t('settings.update.facts.availableVersion')}
            value={version ? `v${version}` : '—'}
            tone={version ? 'accent' : 'neutral'}
          />
        </>
      }
    />
  )
}

/** Build Steam: o cliente Steam baixa e instala o binário (SteamPipe); não há nada a checar aqui. */
function SteamManagedPanel() {
  const { t } = useI18n()
  return (
    <div className="space-y-6">
      <PageHero icon={RotateCw} title={t('settings.update.title')} description={t('settings.update.steam.notice')} />
      <GlassSection
        title={t('settings.update.detailsTitle')}
        icon={Store}
        description={t('settings.update.steam.detail')}
      >
        <GlassCard className="max-w-2xl" hover={false}>
          <p role="status" className="text-base font-semibold text-text-main">
            {t('settings.update.steam.notice')}
          </p>
        </GlassCard>
      </GlassSection>
    </div>
  )
}

type UpdatePanelProps = {
  config: Record<string, unknown>
  saveSetting: (path: string, value: unknown) => Promise<void>
}

export function UpdatePanel({ config, saveSetting }: UpdatePanelProps) {
  const update = useUpdateChecker()
  if (update.channel === 'steam') return <SteamManagedPanel />
  return (
    <div className="space-y-6">
      <DirectUpdatePanel update={update} />
      <AutoUpdateSection config={config} save={saveSetting} />
    </div>
  )
}

/**
 * Changelog da release encontrada — o `notes` do `latest.json`, montado no CI dos commits. Só existe
 * quando o último check achou versão nova (`useUpdateChecker` zera a cada check).
 */
function UpdateNotes({ notes }: { notes: string | null }) {
  const { t } = useI18n()
  if (!notes) return null
  return (
    <section aria-label={t('settings.update.notesTitle')} className="mb-4">
      <h4 className="mb-1.5 text-caption font-bold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.update.notesTitle')}
      </h4>
      <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-xl border border-border-subtle bg-surface-raise p-3 font-sans text-caption leading-relaxed text-text-main">
        {notes}
      </pre>
    </section>
  )
}

function DirectUpdatePanel({ update }: { update: ReturnType<typeof useUpdateChecker> }) {
  const { t } = useI18n()
  const {
    checking,
    available,
    downloading,
    downloaded,
    installing,
    progress,
    version,
    notes,
    error,
    checked,
    ...actions
  } = update
  const [build, setBuild] = useState<BuildIdentity | null>(null)
  const [lastCheckAt, setLastCheckAt] = useState<number | null>(null)
  const wasChecking = useRef(false)

  useEffect(() => {
    let mounted = true
    void (async () => {
      try {
        const health = await getElectronApi().runtime.getHealth()
        if (mounted) setBuild({ version: health.version, packaged: health.packaged })
      } catch {}
    })()
    return () => {
      mounted = false
    }
  }, [])

  useEffect(() => {
    if (wasChecking.current && !checking) setLastCheckAt(Date.now())
    wasChecking.current = checking
  }, [checking])

  const flags = { installing, downloaded, downloading, checking, available, progress, checked, failed: Boolean(error) }
  const statusLabel = statusLabelFor(flags)

  return (
    <div className="space-y-6">
      <UpdateHero flags={flags} build={build} version={version} />
      <GlassSection
        title={t('settings.update.detailsTitle')}
        icon={RefreshCw}
        description={t('settings.update.detailsDescription')}
      >
        {/* Label/value rows across the full content width put a hand-span between each pair. */}
        <GlassCard className="max-w-2xl" hover={false}>
          <div className="mb-4">
            <p role="status" aria-live="polite" className="text-base font-semibold text-text-main">
              {statusLabel}
            </p>
            {(!error || version) && (
              <p className="mt-0.5 text-caption text-text-muted">
                {version
                  ? t('settings.update.newVersionPublished')
                  : checked
                    ? t('settings.update.noPendingVersion')
                    : t('settings.update.clickToCheck')}
              </p>
            )}
          </div>

          <UpdateNotes notes={notes} />

          {downloading &&
            (() => {
              const pct = clampPercent(Math.round(progress))
              return (
                <div className="mb-4 space-y-1.5">
                  <div className="flex justify-between text-caption text-text-muted">
                    <span>{t('settings.update.downloadProgress')}</span>
                    <span className="font-mono">{pct}%</span>
                  </div>
                  <div
                    role="progressbar"
                    aria-valuenow={pct}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label={t('settings.update.downloadProgressAria')}
                    className="h-1.5 w-full overflow-hidden rounded-full bg-surface-raise-strong"
                  >
                    <div
                      className="h-full w-full origin-left rounded-full bg-info transition-transform duration-150"
                      style={{ transform: `scaleX(${Math.max(2, pct) / 100})` }}
                    />
                  </div>
                </div>
              )
            })()}

          <UpdateFacts build={build} lastCheckAt={lastCheckAt} />

          {build && !build.packaged && (
            <p className="mb-3 text-caption leading-relaxed text-text-muted">{t('settings.update.devBuildNotice')}</p>
          )}

          {error && (
            <div
              role="alert"
              className="mb-3 rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-caption text-danger first-letter:uppercase"
            >
              {error}
            </div>
          )}

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void actions.checkForUpdates()}
              disabled={checking || downloading || installing}
              className="flex items-center gap-1.5 rounded-lg border border-border-subtle px-3 py-2 text-caption font-bold text-text-main transition hover:border-info-rim hover:bg-info-soft hover:text-info disabled:opacity-50 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <RefreshCw size={12} className={checking ? 'animate-spin' : ''} aria-hidden="true" />
              {t('settings.update.checkButton')}
            </button>

            {available && !downloaded && (
              <button
                type="button"
                onClick={() => void actions.downloadUpdate()}
                disabled={downloading || installing}
                className="flex items-center gap-1.5 rounded-lg border border-info-rim bg-info-soft px-3 py-2 text-caption font-bold text-info transition hover:bg-info hover:text-white disabled:opacity-50 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <Download size={12} className={downloading ? 'animate-pulse' : ''} aria-hidden="true" />
                {t('settings.update.downloadButton')}
              </button>
            )}

            {downloaded && (
              <button
                type="button"
                onClick={() => void actions.installUpdate()}
                disabled={installing}
                className="flex items-center gap-1.5 rounded-lg border border-ok-rim bg-ok-soft px-3 py-2 text-caption font-bold text-ok transition hover:bg-ok hover:text-white disabled:opacity-50 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <RotateCw size={12} className={installing ? 'animate-spin' : ''} aria-hidden="true" />
                {t('settings.update.installButton')}
              </button>
            )}
          </div>
        </GlassCard>
      </GlassSection>
    </div>
  )
}
