import { type PillVariant, StatusPill } from '@mika/sdk-ui/settings'
import { Activity, RefreshCw } from 'lucide-react'
import type { LlmStatus, LlmTimingSeries } from '../../../shared/electron-api-system-types'
import { useI18n } from '../../src/hooks/useI18n'
import { formatBytes, formatMs, gpuOffload } from './use-llm-status'

type SampleKey = 'firstTokenMs' | 'firstAudioMs' | 'totalMs'

function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return null
  const max = Math.max(...values)
  const min = Math.min(...values)
  const span = max - min || 1
  const points = values
    .map((v, i) => `${((i / (values.length - 1)) * 100).toFixed(1)},${(26 - ((v - min) / span) * 22).toFixed(1)}`)
    .join(' ')
  return (
    <svg viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true" className="mt-2 h-7 w-full text-accent">
      <polyline
        points={points}
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  )
}

function LatencyTile({ label, series, values }: { label: string; series: LlmTimingSeries; values: number[] }) {
  const { t } = useI18n()
  return (
    <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2.5">
      <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">{label}</p>
      <p className="mt-1 font-display text-lg font-bold tracking-tight text-text-main">{formatMs(series.last)}</p>
      <p className="text-caption text-text-muted">
        {series.count > 0
          ? t('settings.llm.live.median')
              .replace('{{value}}', formatMs(series.median))
              .replace('{{count}}', String(series.count))
          : '—'}
      </p>
      <Sparkline values={values} />
    </div>
  )
}

type Pill = { variant: PillVariant; label: string; title?: string }

/** Pílula de GPU do modelo carregado: parcial ou só CPU é o motivo usual de um turno lento. */
function offloadPill(loaded: LlmStatus['ollama']['loaded'][number], t: ReturnType<typeof useI18n>['t']): Pill[] {
  const offload = gpuOffload(loaded)
  if (!offload) return []
  const title = t('settings.llm.live.offloadHint')
  if (offload.kind === 'gpu') return [{ variant: 'ok', label: t('settings.llm.live.offloadGpu'), title }]
  if (offload.kind === 'cpu') return [{ variant: 'warn', label: t('settings.llm.live.offloadCpu'), title }]
  return [
    {
      variant: 'warn',
      label: t('settings.llm.live.offloadPartial').replace('{{pct}}', String(offload.percent)),
      title,
    },
  ]
}

function statusPills(status: LlmStatus, t: ReturnType<typeof useI18n>['t']): Pill[] {
  const { ollama } = status
  if (status.provider !== 'ollama') {
    return [
      status.ready
        ? { variant: 'ok', label: t('settings.llm.live.credentialOk') }
        : { variant: 'warn', label: t('settings.llm.live.credentialMissing') },
    ]
  }
  if (!ollama.reachable) {
    return [{ variant: 'error', label: t('settings.llm.live.ollamaOffline'), title: ollama.error ?? undefined }]
  }
  const loaded = ollama.loaded[0]
  return [
    {
      variant: 'ok',
      label: t('settings.llm.live.ollamaOnline').replace('{{version}}', ollama.version ?? ''),
      title: ollama.endpoint,
    },
    {
      variant: 'neutral',
      label: t('settings.llm.live.installed').replace('{{count}}', String(ollama.installed.length)),
    },
    loaded
      ? {
          variant: 'accent',
          label: t('settings.llm.live.loaded')
            .replace('{{model}}', loaded.name)
            .replace('{{size}}', formatBytes(loaded.sizeVramBytes ?? loaded.sizeBytes)),
        }
      : { variant: 'inactive', label: t('settings.llm.live.loadedNone') },
    ...(loaded ? offloadPill(loaded, t) : []),
  ]
}

type Props = { status: LlmStatus | null; loading: boolean; onRefresh: () => void; compact?: boolean }

/** "Status ao vivo": o Ollama responde, o que está na VRAM e quanto a Mika demora por turno. */
export function LlmLiveStatus({ status, loading, onRefresh, compact = false }: Props) {
  const { t } = useI18n()
  const samples = status?.timings.samples ?? []
  const valuesOf = (key: SampleKey) => samples.flatMap((s) => (s[key] == null ? [] : [s[key] as number]))
  const tiles: { key: SampleKey; label: string; series?: LlmTimingSeries }[] = [
    { key: 'firstTokenMs', label: t('settings.llm.live.firstToken'), series: status?.timings.firstToken },
    { key: 'firstAudioMs', label: t('settings.llm.live.firstAudio'), series: status?.timings.firstAudio },
    { key: 'totalMs', label: t('settings.llm.live.total'), series: status?.timings.total },
  ]
  const measured = samples.length > 0
  return (
    <section aria-label={t('settings.llm.live.title')} className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="inline-flex items-center gap-1.5 text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
          <Activity size={12} aria-hidden="true" className={status?.ready ? 'text-ok' : 'text-warn'} />
          {t('settings.llm.live.title')}
        </span>
        {status ? (
          statusPills(status, t).map((pill) => (
            <StatusPill key={pill.label} variant={pill.variant} label={pill.label} title={pill.title} dot />
          ))
        ) : (
          <StatusPill variant="loading" />
        )}
        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          aria-label={t('settings.llm.live.refresh')}
          title={t('settings.llm.live.refresh')}
          className="ml-auto inline-flex h-7 w-7 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-muted hover:text-text-main disabled:opacity-60"
        >
          <RefreshCw size={12} className={loading ? 'animate-spin' : undefined} />
        </button>
      </div>
      {status && status.provider === 'ollama' && !status.ollama.reachable && !compact && (
        <p className="text-caption text-text-muted">{t('settings.llm.live.offlineHint')}</p>
      )}
      {measured ? (
        <div className="grid gap-2 sm:grid-cols-3">
          {tiles.map((tile) => (
            <LatencyTile
              key={tile.key}
              label={tile.label}
              series={tile.series ?? { last: null, median: null, count: 0 }}
              values={valuesOf(tile.key)}
            />
          ))}
        </div>
      ) : (
        <p className="text-caption text-text-muted">{t('settings.llm.live.noTurns')}</p>
      )}
    </section>
  )
}
