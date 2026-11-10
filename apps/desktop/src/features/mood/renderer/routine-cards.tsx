import type { TranslationDict } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { ConfirmButton, GlassCard, GlassSlider, GlassToggle, SectionHeader, StatusPill } from '@mika/sdk-ui/settings'
import { Coffee, Droplet, Gamepad2, MonitorSmartphone, Moon, Trash2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { configBool, configNum } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'

type Needs = { sleepiness: number; thirst: number; hunger: number; boredom: number }
type NeedsData = { enabled?: boolean; needs?: Needs | null; awakeHours?: number | null }
type Entry = { name: string; seconds: number }
type HabitsData = {
  enabled?: boolean
  platformSupported?: boolean
  programs7d?: Entry[]
  programs30d?: Entry[]
  games30d?: Entry[]
}
type ToolResult<T> = { success?: boolean; data?: T; error?: string }

const REFRESH_MS = 30_000

async function runTool<T>(name: string, args: Record<string, unknown>): Promise<T | undefined> {
  const result = (await getElectronApi().runtime.executeTool(name, args)) as ToolResult<T>
  if (result.success === false) throw new Error(result.error ?? name)
  return result.data
}

/** Live data from a tool, re-read every `REFRESH_MS` — bars move while the panel is open. */
function useToolData<T>(name: string, errorKey: keyof TranslationDict) {
  const { t } = useI18n()
  const [data, setData] = useState<T | undefined>()
  const [error, setError] = useState<string | null>(null)
  const refresh = useCallback(async () => {
    try {
      setData(await runTool<T>(name, { action: 'get' }))
      setError(null)
    } catch (err) {
      setError(toDisplayError(err, t(errorKey)))
    }
  }, [name, errorKey, t])
  useEffect(() => {
    void refresh()
    const id = window.setInterval(() => void refresh(), REFRESH_MS)
    return () => window.clearInterval(id)
  }, [refresh])
  return { data, error, refresh, setError }
}

function Meter({ label, value, max = 1, detail }: { label: string; value: number; max?: number; detail?: string }) {
  const percent = max > 0 ? Math.round(Math.min(1, value / max) * 100) : 0
  const tone = max === 1 && value >= 0.75 ? 'bg-warn' : 'bg-accent'
  return (
    <div className="min-w-0">
      <div className="mb-1 flex items-baseline justify-between gap-2 text-xs">
        <span className="truncate font-semibold text-text-main">{label}</span>
        <span className="shrink-0 tabular-nums text-text-muted">{detail ?? `${percent}%`}</span>
      </div>
      <div
        role="progressbar"
        aria-label={label}
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
        className="h-2 w-full overflow-hidden rounded-full bg-surface-raise"
      >
        <div
          className={`h-full rounded-full ${tone} transition-[width] duration-500`}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  )
}

const NEED_BARS: { key: keyof Needs; labelKey: keyof TranslationDict }[] = [
  { key: 'sleepiness', labelKey: 'settings.mood.routine.sleepiness' },
  { key: 'thirst', labelKey: 'settings.mood.routine.thirst' },
  { key: 'hunger', labelKey: 'settings.mood.routine.hunger' },
  { key: 'boredom', labelKey: 'settings.mood.routine.boredom' },
]

const CARE_ACTIONS = [
  { need: 'drink', icon: Droplet, labelKey: 'settings.mood.routine.drink' },
  { need: 'eat', icon: Coffee, labelKey: 'settings.mood.routine.eat' },
  { need: 'sleep', icon: Moon, labelKey: 'settings.mood.routine.sleep' },
] as const

const ACTION_BUTTON_CLASS =
  'flex h-9 items-center justify-center gap-2 rounded-xl border border-border-subtle bg-surface-raise px-3 text-xs font-semibold text-text-main transition hover:border-border-strong hover:bg-surface-raise-strong disabled:cursor-not-allowed disabled:opacity-50'

interface CardProps {
  config: Record<string, unknown>
  masterEnabled: boolean
  saveBool: (path: string, value: boolean) => Promise<void>
  saveNum: (path: string, value: number) => Promise<void>
}

export function NeedsCard({ config, masterEnabled, saveBool, saveNum }: CardProps) {
  const { t } = useI18n()
  const needsEnabled = configBool(config, 'mood.needsEnabled', true)
  const intensity = configNum(config, 'mood.needsIntensity', 1)
  const { data, error, refresh, setError } = useToolData<NeedsData>('needs', 'settings.mood.routine.error')
  const [busy, setBusy] = useState<string | null>(null)
  const live = masterEnabled && needsEnabled

  const care = async (need: string) => {
    setBusy(need)
    try {
      await runTool('needs', { action: 'satisfy', need })
      await refresh()
    } catch (err) {
      setError(toDisplayError(err, t('settings.mood.routine.error')))
    } finally {
      setBusy(null)
    }
  }

  return (
    <GlassCard span="full">
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <SectionHeader title={t('settings.mood.routine.title')} description={t('settings.mood.routine.description')} />
        {typeof data?.awakeHours === 'number' && live && (
          <StatusPill
            variant="info"
            label={t('settings.mood.routine.awake').replace('{{count}}', String(data.awakeHours))}
          />
        )}
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <GlassToggle
          label={t('settings.mood.field.needsEnabled.label')}
          description={t('settings.mood.field.needsEnabled.description')}
          configPath="mood.needsEnabled"
          value={needsEnabled}
          onSave={(_p, v) => saveBool('mood.needsEnabled', v as boolean)}
          disabled={!masterEnabled}
        />
        <GlassSlider
          label={t('settings.mood.field.needsIntensity.label')}
          description={t('settings.mood.field.needsIntensity.helperText')}
          configPath="mood.needsIntensity"
          value={intensity}
          min={0}
          max={2}
          step={0.1}
          showValue={(v) => `${Math.round(v * 100)}%`}
          onSave={saveNum}
          disabled={!live}
        />
      </div>
      {error && (
        <div role="alert" className="mt-3 rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs text-warn">
          {error}
        </div>
      )}
      {live && data?.needs ? (
        <>
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            {NEED_BARS.map(({ key, labelKey }) => (
              <Meter key={key} label={t(labelKey)} value={data.needs?.[key] ?? 0} />
            ))}
          </div>
          <div className="mt-4 flex flex-wrap gap-2">
            {CARE_ACTIONS.map(({ need, icon: Icon, labelKey }) => (
              <button
                key={need}
                type="button"
                disabled={busy !== null}
                onClick={() => void care(need)}
                className={ACTION_BUTTON_CLASS}
              >
                <Icon className={`h-3.5 w-3.5 ${busy === need ? 'animate-pulse' : ''}`} aria-hidden="true" />
                {t(labelKey)}
              </button>
            ))}
          </div>
        </>
      ) : (
        <p role="status" className="mt-4 text-caption text-text-muted">
          {t('settings.mood.routine.off')}
        </p>
      )}
    </GlassCard>
  )
}

function formatDuration(seconds: number): string {
  const minutes = Math.round(seconds / 60)
  return minutes < 60 ? `${minutes}min` : `${Math.round(minutes / 6) / 10}h`
}

function RankedList({
  title,
  icon: Icon,
  entries,
  empty,
}: {
  title: string
  icon: typeof Gamepad2
  entries: Entry[]
  empty: string
}) {
  const max = entries[0]?.seconds ?? 0
  return (
    <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise p-3">
      <p className="mb-3 flex items-center gap-2 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
        <Icon className="h-3.5 w-3.5" aria-hidden="true" />
        {title}
      </p>
      {entries.length === 0 ? (
        <p className="text-caption text-text-muted">{empty}</p>
      ) : (
        <div className="flex flex-col gap-2.5">
          {entries.map((entry) => (
            <Meter
              key={entry.name}
              label={entry.name}
              value={entry.seconds}
              max={max}
              detail={formatDuration(entry.seconds)}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export function HabitsCard({ config, masterEnabled, saveBool }: Omit<CardProps, 'saveNum'>) {
  const { t } = useI18n()
  const habitsEnabled = configBool(config, 'mood.habitsEnabled', true)
  const { data, error, refresh, setError } = useToolData<HabitsData>('habits', 'settings.mood.habits.error')
  const [range, setRange] = useState<7 | 30>(7)
  const programs = (range === 7 ? data?.programs7d : data?.programs30d) ?? []

  const clear = async () => {
    try {
      await runTool('habits', { action: 'clear' })
      await refresh()
    } catch (err) {
      setError(toDisplayError(err, t('settings.mood.habits.error')))
    }
  }

  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.mood.habits.title')}
        description={t('settings.mood.habits.description')}
        className="mb-4"
      />
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0 flex-1">
          <GlassToggle
            label={t('settings.mood.field.habitsEnabled.label')}
            description={t('settings.mood.field.habitsEnabled.description')}
            configPath="mood.habitsEnabled"
            value={habitsEnabled}
            onSave={(_p, v) => saveBool('mood.habitsEnabled', v as boolean)}
            disabled={!masterEnabled}
          />
        </div>
        <ConfirmButton
          label={t('settings.mood.habits.clear')}
          ariaLabel={t('settings.mood.habits.clear')}
          icon={Trash2}
          onConfirm={() => void clear()}
        />
      </div>
      {error && (
        <div role="alert" className="mt-3 rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs text-warn">
          {error}
        </div>
      )}
      {data?.platformSupported === false && (
        <p role="status" className="mt-3 text-caption text-text-muted">
          {t('settings.mood.habits.platform')}
        </p>
      )}
      <div className="mt-4 flex gap-2">
        {([7, 30] as const).map((days) => (
          <button
            key={days}
            type="button"
            aria-pressed={range === days}
            onClick={() => setRange(days)}
            className={
              range === days
                ? ACTION_BUTTON_CLASS.replace(
                    'border-border-subtle bg-surface-raise',
                    'border-accent-rim bg-accent-soft',
                  )
                : ACTION_BUTTON_CLASS
            }
          >
            {t(days === 7 ? 'settings.mood.habits.range7' : 'settings.mood.habits.range30')}
          </button>
        ))}
      </div>
      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        <RankedList
          title={t('settings.mood.habits.programs')}
          icon={MonitorSmartphone}
          entries={programs}
          empty={t('settings.mood.habits.empty')}
        />
        <RankedList
          title={t('settings.mood.habits.games')}
          icon={Gamepad2}
          entries={data?.games30d ?? []}
          empty={t('settings.mood.habits.empty')}
        />
      </div>
    </GlassCard>
  )
}
