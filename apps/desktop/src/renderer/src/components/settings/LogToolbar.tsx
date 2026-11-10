import type { LogLevel } from '@mika/sdk/core/logger-format'
import type { TranslationDict } from '@mika/sdk/i18n'
import { AuroraField } from '@mika/sdk-ui/settings'
import { Download, Pause, Play, Trash2, X } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import type { AudienceFilter } from './LogsPanel'
import { ConfirmButton } from './RichSettingBlocks'

const LEVELS: LogLevel[] = ['debug', 'info', 'warn', 'error']
const LEVEL_ACTIVE: Record<LogLevel, string> = {
  debug: 'border-ok-rim bg-ok-soft text-ok',
  info: 'border-info-rim bg-info-soft text-info',
  warn: 'border-warn-rim bg-warn-soft text-warn',
  error: 'border-danger-rim bg-danger-soft text-danger',
}

/** The single level control: saves `system.logLevel` (what gets recorded) and is the view's floor. */
function LevelControl({ level, onChange }: { level: LogLevel; onChange: (l: LogLevel) => void }) {
  const { t } = useI18n()
  return (
    <fieldset
      title={t('settings.logs.levelConfig.description')}
      className="m-0 flex shrink-0 items-center gap-0.5 rounded-xl border border-border-subtle bg-surface-raise/40 p-0.5"
    >
      <legend className="sr-only">{t('settings.logs.levelConfig.title')}</legend>
      {LEVELS.map((value) => {
        const active = value === level
        return (
          <button
            key={value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(value)}
            className={`flex items-center gap-1 rounded-lg border px-2 py-1.5 text-micro font-bold uppercase tracking-wider transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
              active ? LEVEL_ACTIVE[value] : 'border-transparent text-text-muted hover:text-text-main'
            }`}
          >
            {t(`settings.logs.level.${value}`)}
          </button>
        )
      })}
    </fieldset>
  )
}

type TKey = keyof TranslationDict

function audienceTabs(t: (key: TKey) => string): Array<{ value: AudienceFilter; label: string; title: string }> {
  return [
    { value: 'human', label: t('settings.logs.audience.human'), title: t('settings.logs.audience.humanTitle') },
    { value: 'ai', label: t('settings.logs.audience.ai'), title: t('settings.logs.audience.aiTitle') },
    { value: 'all', label: t('settings.logs.audience.all'), title: t('settings.logs.audience.allTitle') },
  ]
}

export function LogToolbar({
  searchText,
  onSearchChange,
  level,
  onLevelChange,
  isPaused,
  onTogglePause,
  onExport,
  onClear,
  hasLogs,
  audienceFilter,
  onAudienceFilterChange,
  traceFilter,
  onClearTraceFilter,
}: {
  searchText: string
  onSearchChange: (v: string) => void
  level: LogLevel
  onLevelChange: (l: LogLevel) => void
  isPaused: boolean
  onTogglePause: () => void
  onExport: () => void
  onClear: () => void
  hasLogs: boolean
  audienceFilter: AudienceFilter
  onAudienceFilterChange: (a: AudienceFilter) => void
  traceFilter: string | null
  onClearTraceFilter: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <div className="flex shrink-0 items-center gap-1 rounded-xl border border-border-subtle bg-surface-raise/40 p-0.5">
          {audienceTabs(t).map(({ value, label, title }) => (
            <button
              key={value}
              type="button"
              onClick={() => onAudienceFilterChange(value)}
              title={title}
              className={`rounded-lg px-2.5 py-1.5 text-micro font-bold uppercase tracking-wider transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                audienceFilter === value
                  ? 'border border-accent-rim bg-accent-soft text-accent'
                  : 'border border-transparent text-text-muted hover:text-text-main'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        {traceFilter && (
          <button
            type="button"
            onClick={onClearTraceFilter}
            className="flex shrink-0 items-center gap-1 rounded-xl border border-accent-rim bg-accent-soft px-2 py-1.5 font-mono text-caption text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            title={t('settings.logs.traceFilter.showAllTitle')}
          >
            {t('settings.logs.traceFilter.prefix')} {traceFilter}
            <X aria-hidden="true" size={12} />
          </button>
        )}
        <div className="min-w-40 flex-1">
          <AuroraField
            label={t('settings.logs.searchPlaceholder')}
            hideLabel
            type="search"
            value={searchText}
            onChange={onSearchChange}
            placeholder={t('settings.logs.searchPlaceholder')}
          />
        </div>

        <LevelControl level={level} onChange={onLevelChange} />
      </div>

      <div className="flex items-center gap-1.5 justify-end">
        <button
          type="button"
          onClick={onTogglePause}
          className={`flex items-center gap-1.5 min-h-8 rounded-lg border border-border-subtle bg-surface-raise/40 px-2.5 py-1.5 text-caption font-semibold transition duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
            isPaused ? 'text-warn hover:bg-warn-soft' : 'text-text-muted hover:bg-surface-raise'
          }`}
          title={isPaused ? t('settings.logs.pause.resumeTitle') : t('settings.logs.pause.pauseTitle')}
        >
          {isPaused ? <Play size={11} aria-hidden="true" /> : <Pause size={11} aria-hidden="true" />}
          {isPaused ? t('settings.logs.pause.resume') : t('settings.logs.pause.pause')}
        </button>

        <button
          type="button"
          onClick={onExport}
          disabled={!hasLogs}
          className="flex items-center gap-1.5 min-h-8 rounded-lg border border-border-subtle bg-surface-raise/40 px-2.5 py-1.5 text-caption font-semibold text-text-muted hover:bg-surface-raise disabled:opacity-50 disabled:cursor-not-allowed transition duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          title={t('settings.logs.export.title')}
        >
          <Download size={11} aria-hidden="true" />
          {t('settings.logs.export.label')}
        </button>

        <span aria-hidden="true" className="mx-1 h-6 w-px shrink-0 bg-surface-raise-strong" />

        <ConfirmButton
          icon={Trash2}
          label={t('settings.logs.clear.label')}
          ariaLabel={t('settings.logs.clear.ariaLabel')}
          confirmLabel={t('settings.logs.clear.label')}
          onConfirm={onClear}
        />
      </div>
    </div>
  )
}
