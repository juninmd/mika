import type { TranslationDict } from '@mika/sdk/i18n'
import { ConfirmButton, formatLocalDate, formatLocalNumber, GlassCard, GlassSection } from '@mika/sdk-ui/settings'
import { Coins, Eraser, RefreshCw } from 'lucide-react'
import { useCallback, useState } from 'react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { useI18n } from '../../src/hooks/useI18n'
import { useVisiblePolling } from '../../src/hooks/useVisiblePolling'
import { getElectronApi } from '../../src/lib/shell-api'

type TokenUsage = {
  totalTokens: number
  totalPromptTokens: number
  totalCompletionTokens: number
  totalCostUsd: number

  unknownCostTurns?: number

  totalCachedPromptTokens?: number
  totalInteractions: number
  firstUsedAt: number
  lastUsedAt: number
}

function sameStats(prev: TokenUsage | null, next: TokenUsage | null): boolean {
  if (Object.is(prev, next)) return true
  if (!prev || !next) return false
  return (
    prev.totalTokens === next.totalTokens &&
    prev.totalPromptTokens === next.totalPromptTokens &&
    prev.totalCompletionTokens === next.totalCompletionTokens &&
    prev.totalCostUsd === next.totalCostUsd &&
    prev.unknownCostTurns === next.unknownCostTurns &&
    prev.totalCachedPromptTokens === next.totalCachedPromptTokens &&
    prev.totalInteractions === next.totalInteractions &&
    prev.firstUsedAt === next.firstUsedAt &&
    prev.lastUsedAt === next.lastUsedAt
  )
}

function formatNumber(n: number, locale: string): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return formatLocalNumber(n, locale)
}

function MetricCard({ label, value, color }: { label: string; value: string | number; color?: string }) {
  return (
    <GlassCard>
      <p className="text-micro uppercase tracking-widest text-text-muted">{label}</p>
      <p className="text-lg font-bold tabular-nums tracking-tight" style={color ? { color } : undefined}>
        {value}
      </p>
    </GlassCard>
  )
}

const POLL_INTERVAL_MS = 3000

function buildMainMetrics(stats: TokenUsage | null, locale: string, t: (key: keyof TranslationDict) => string) {
  const accent = 'var(--accent-text)'
  const unknownTurns = stats?.unknownCostTurns ?? 0
  return [
    {
      label: t('settings.llm.panel.tokenUsage.totalTokens'),
      value: formatNumber(stats?.totalTokens ?? 0, locale),
      color: accent,
    },
    {
      label: t('settings.llm.panel.tokenUsage.prompt'),
      value: formatNumber(stats?.totalPromptTokens ?? 0, locale),
      color: accent,
    },
    {
      label: t('settings.llm.panel.tokenUsage.completion'),
      value: formatNumber(stats?.totalCompletionTokens ?? 0, locale),
      color: accent,
    },
    {
      label:
        unknownTurns > 0
          ? t('settings.llm.panel.tokenUsage.partialCost')
          : t('settings.llm.panel.tokenUsage.estimatedCost'),
      value: `${unknownTurns > 0 ? '≥ ' : ''}$${(stats?.totalCostUsd ?? 0).toFixed(4)}`,
      color: accent,
    },
  ]
}

function cacheHitLabel(stats: TokenUsage | null): string {
  const prompt = stats?.totalPromptTokens ?? 0
  if (prompt <= 0) return '—'
  const cached = stats?.totalCachedPromptTokens
  if (cached === undefined) return '—'
  return `${((cached / prompt) * 100).toFixed(1)}%`
}

function buildUsageMetrics(stats: TokenUsage | null, locale: string, t: (key: keyof TranslationDict) => string) {
  const date = (at: number | undefined) => (at ? formatLocalDate(at, locale) : '—')
  return [
    { label: t('settings.llm.panel.tokenUsage.promptCache'), value: cacheHitLabel(stats) },
    { label: t('settings.llm.panel.tokenUsage.interactions'), value: stats?.totalInteractions ?? 0 },
    { label: t('settings.llm.panel.tokenUsage.firstUse'), value: date(stats?.firstUsedAt) },
    { label: t('settings.llm.panel.tokenUsage.lastUse'), value: date(stats?.lastUsedAt) },
  ]
}

export function TokenUsagePanel(_props: FeatureComponentProps) {
  const { t, locale } = useI18n()
  const [stats, setStats] = useState<TokenUsage | null>(null)
  const [loading, setLoading] = useState(true)

  const load = useCallback(async (silent?: boolean) => {
    if (!silent) setLoading(true)
    try {
      const result = await getElectronApi().runtime.getTokenUsage()
      setStats((prev) => (sameStats(prev, result) ? prev : result))
    } catch {
      setStats(null)
    } finally {
      if (!silent) setLoading(false)
    }
  }, [])

  useVisiblePolling(load, POLL_INTERVAL_MS)

  async function handleReset() {
    await getElectronApi().runtime.resetTokenUsage()
    void load()
  }

  const now = Date.now()
  const hasData = stats && stats.totalInteractions > 0
  const unknownTurns = stats?.unknownCostTurns ?? 0
  const mainMetrics = buildMainMetrics(stats, locale, t)
  const usageMetrics = buildUsageMetrics(stats, locale, t)

  return (
    <GlassSection
      title={t('settings.llm.panel.tokenUsage.title')}
      icon={Coins}
      badge={t('settings.llm.panel.tokenUsage.badge')}
    >
      <div className="space-y-4">
        {loading ? (
          <div className="flex items-center justify-center py-8">
            <RefreshCw size={18} className="animate-spin text-warn/60" />
          </div>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              {mainMetrics.map((metric) => (
                <MetricCard key={metric.label} {...metric} />
              ))}
            </div>

            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              {usageMetrics.map((metric) => (
                <MetricCard key={metric.label} {...metric} />
              ))}
            </div>

            {unknownTurns > 0 && (
              <div className="rounded-lg border border-warn-rim bg-warn-soft px-4 py-3 text-caption text-warn/90">
                {unknownTurns === 1
                  ? `1 ${t('settings.llm.panel.tokenUsage.unknownCostSingular')}`
                  : `${unknownTurns} ${t('settings.llm.panel.tokenUsage.unknownCostPlural')}`}
              </div>
            )}

            {!hasData && (
              <div className="rounded-lg border border-border-subtle bg-surface-raise px-4 py-3 text-caption text-text-muted">
                {t('settings.llm.panel.tokenUsage.noDataYet')}
              </div>
            )}

            {now - (stats?.lastUsedAt ?? 0) < 5000 && (
              <p className="text-caption text-warn/60">{t('settings.llm.panel.tokenUsage.sessionActive')}</p>
            )}

            {hasData && (
              <div className="flex gap-2">
                <ConfirmButton
                  ariaLabel={t('settings.llm.panel.tokenUsage.resetAriaLabel')}
                  label={t('settings.llm.panel.tokenUsage.resetLabel')}
                  confirmLabel={t('settings.llm.panel.tokenUsage.resetConfirmLabel')}
                  icon={Eraser}
                  onConfirm={handleReset}
                />
              </div>
            )}
          </>
        )}
      </div>
    </GlassSection>
  )
}
