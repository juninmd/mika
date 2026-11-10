import type { TranslationDict } from '@mika/sdk/i18n'
import { bipolarToPercent } from '@mika/sdk/utils/clamp'
import { toDisplayError } from '@mika/sdk-ui'
import { GlassCard } from '@mika/sdk-ui/settings'
import { Heart, RefreshCw, Sparkles, Zap } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { moodName } from '../../../renderer/src/components/mood-hud-config'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'

type AffinityLevelKey = 'new' | 'known' | 'friend' | 'fan' | 'bestie'
type MoodState = { label?: string; valence?: number; arousal?: number }
type AffinityState = { label?: AffinityLevelKey; score?: number; level?: number }
type MoodToolResult = { success?: boolean; data?: { mood?: MoodState; affinity?: AffinityState }; error?: string }

const AFFINITY_LEVEL_LABEL_KEYS: Record<AffinityLevelKey, keyof TranslationDict> = {
  new: 'mood.affinity.level.new',
  known: 'mood.affinity.level.known',
  friend: 'mood.affinity.level.friend',
  fan: 'mood.affinity.level.fan',
  bestie: 'mood.affinity.level.bestie',
}

function formatPercent(value?: number): string {
  if (typeof value !== 'number') return '0%'
  return `${Math.round(bipolarToPercent(value))}%`
}

const NUDGE_BUTTON_CLASS =
  'flex h-10 items-center justify-center gap-2 rounded-xl border border-border-subtle bg-surface-raise text-xs font-semibold text-text-main transition hover:border-border-strong hover:bg-surface-raise-strong disabled:cursor-not-allowed disabled:opacity-50'

/** Valence (-1..1) as words — the raw "+0.48" meant nothing to anyone but the engine. */
function valenceKey(value?: number): keyof TranslationDict {
  const v = typeof value === 'number' ? value : 0
  if (v >= 0.5) return 'settings.mood.valence.veryPositive'
  if (v >= 0.15) return 'settings.mood.valence.positive'
  if (v > -0.15) return 'settings.mood.valence.neutral'
  if (v > -0.5) return 'settings.mood.valence.negative'
  return 'settings.mood.valence.veryNegative'
}

type NudgeKind = 'bond' | 'hype' | 'calm'

const NUDGES: {
  kind: NudgeKind
  icon: typeof Heart
  labelKey: keyof TranslationDict
  hintKey: keyof TranslationDict
}[] = [
  { kind: 'bond', icon: Sparkles, labelKey: 'settings.mood.live.bond', hintKey: 'settings.mood.nudge.bondHint' },
  { kind: 'hype', icon: Zap, labelKey: 'settings.mood.live.hype', hintKey: 'settings.mood.nudge.hypeHint' },
  { kind: 'calm', icon: Heart, labelKey: 'settings.mood.live.calm', hintKey: 'settings.mood.nudge.calmHint' },
]

export function LiveMoodCard() {
  const { t } = useI18n()
  const [mood, setMood] = useState<MoodState | null>(null)
  const [affinity, setAffinity] = useState<AffinityState | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = (await getElectronApi().runtime.executeTool('mood', { action: 'get' })) as MoodToolResult
      if (result.success === false) throw new Error(result.error ?? t('settings.mood.live.errorState'))
      setMood(result.data?.mood ?? null)
      setAffinity(result.data?.affinity ?? null)
    } catch (err) {
      setError(toDisplayError(err, t('settings.mood.live.errorRead')))
    } finally {
      setLoading(false)
    }
  }, [t])

  const nudge = useCallback(
    async (kind: NudgeKind) => {
      setSaving(kind)
      setError(null)
      try {
        const result = (await (kind === 'bond'
          ? getElectronApi().runtime.executeTool('mood', {
              action: 'register_interaction',
              weight: 2,
              reason: 'manual:settings-bond',
            })
          : getElectronApi().runtime.executeTool('mood', {
              action: 'update',
              valence: kind === 'hype' ? 0.55 : 0.25,
              arousal: kind === 'hype' ? 0.65 : -0.25,
            }))) as MoodToolResult
        if (result.success === false) throw new Error(result.error ?? t('settings.mood.live.errorState'))
        await refresh()
      } catch (err) {
        setError(toDisplayError(err, t('settings.mood.live.errorAdjust')))
      } finally {
        setSaving(null)
      }
    },
    [refresh, t],
  )

  useEffect(() => {
    void refresh()
  }, [refresh])

  return (
    <GlassCard span="full">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent">
            <Sparkles size={17} aria-hidden="true" />
          </div>
          <div>
            <h4 className="text-sm font-semibold text-text-main">{t('settings.mood.live.title')}</h4>
            <p className="text-xs text-text-muted">{t('settings.mood.live.badge')}</p>
          </div>
        </div>
        <button
          type="button"
          aria-label={t('settings.mood.live.refresh')}
          onClick={() => void refresh()}
          className="rounded-lg border border-border-subtle bg-surface-raise p-2 text-text-muted transition hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {error && (
        <div role="alert" className="mb-3 rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs text-warn">
          {error}
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-3">
        <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise p-3">
          <p className="text-micro font-bold uppercase tracking-widest text-text-muted">
            {t('settings.mood.live.moodLabel')}
          </p>
          <p className="mt-1 truncate text-base font-black text-text-main">
            {mood?.label ? moodName(mood.label, t) : t('settings.mood.live.moodUndefined')}
          </p>
          <p className="mt-1 text-caption text-text-muted" title={t('settings.mood.valence.hint')}>
            {t('settings.mood.valence.prefix')} {t(valenceKey(mood?.valence))}
          </p>
        </div>
        <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise p-3">
          <p className="text-micro font-bold uppercase tracking-widest text-text-muted">
            {t('settings.mood.live.energyLabel')}
          </p>
          <p className="mt-1 truncate text-base font-black text-text-main">{formatPercent(mood?.arousal)}</p>
          <p className="mt-1 text-caption text-text-muted">{t('settings.mood.live.activation')}</p>
        </div>
        <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise p-3">
          <p className="text-micro font-bold uppercase tracking-widest text-text-muted">
            {t('settings.mood.live.affinityLabel')}
          </p>
          <p className="mt-1 truncate text-base font-black text-text-main">
            {affinity?.label ? t(AFFINITY_LEVEL_LABEL_KEYS[affinity.label]) : t('settings.mood.live.affinityNew')}
          </p>
          <p className="mt-1 text-caption text-text-muted">{`${Math.round(affinity?.score ?? 0)} ${t('settings.mood.live.pts')}`}</p>
        </div>
      </div>

      <div className="mt-4 grid gap-2 sm:grid-cols-3">
        {NUDGES.map(({ kind, icon: Icon, labelKey, hintKey }) => (
          <div key={kind} className="flex flex-col gap-1.5">
            <button
              type="button"
              disabled={saving !== null}
              onClick={() => void nudge(kind)}
              title={t(hintKey)}
              aria-describedby={`mood-nudge-${kind}`}
              className={NUDGE_BUTTON_CLASS}
            >
              <Icon className={`h-3.5 w-3.5 ${saving === kind ? 'animate-pulse' : ''}`} aria-hidden="true" />
              {t(labelKey)}
            </button>
            <p id={`mood-nudge-${kind}`} className="px-1 text-caption leading-snug text-text-muted">
              {t(hintKey)}
            </p>
          </div>
        ))}
      </div>
    </GlassCard>
  )
}
