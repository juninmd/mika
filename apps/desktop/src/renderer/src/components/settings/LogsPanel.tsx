import { FolderOpen, ScrollText, Terminal } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { revealAppFolder } from '../../lib/shell-links'
import { GlassSection, LazyList, PageHero, StatusPill } from './ds'
import { LogLine, LogToolbar } from './logs-panel-components'
import { MAX_LOGS } from './logs-panel-filters'
import { useLogStream } from './use-log-stream'

export type { AudienceFilter } from './logs-panel-filters'

type Props = {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
}

export function LogsPanel({ config, onSave }: Props) {
  const { t } = useI18n()
  const {
    audienceFilter,
    setAudienceFilter,
    traceFilter,
    setTraceFilter,
    searchText,
    setSearchText,
    isPaused,
    setIsPaused,
    autoScroll,
    loadError,
    containerRef,
    currentLogLevel,
    handleScroll,
    filteredLogs,
    handleLogLevelChange,
    clearLogs,
    retryLoad,
    isTruncated,
    totalCaptured,
    exportLogs,
  } = useLogStream({ config, onSave, t })

  return (
    <div className="space-y-6">
      <PageHero
        icon={ScrollText}
        title={t('settings.logs.header.title')}
        description={t('settings.logs.header.description')}
        pills={
          <>
            <StatusPill variant="neutral" label={t('settings.logs.header.badge')} />
            <StatusPill
              variant={isPaused ? 'warn' : 'ok'}
              label={isPaused ? t('settings.logs.streamingPaused') : t('settings.logs.header.live')}
              dot
            />
            <StatusPill variant="info" label={`${t('settings.logs.header.levelPrefix')} ${currentLogLevel}`} />
          </>
        }
        actions={
          <button
            type="button"
            onClick={() => void revealAppFolder('logs', t('settings.logs.openFolderError'))}
            className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-3 text-xs font-semibold text-text-main transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <FolderOpen className="size-3.5" aria-hidden="true" />
            {t('settings.logs.openFolder')}
          </button>
        }
      />
      <GlassSection
        title={t('settings.logs.systemOutput')}
        icon={Terminal}
        description={t('settings.logs.header.streamDescription')}
      >
        <div className="space-y-4">
          <div className="flex flex-col gap-2">
            <LogToolbar
              searchText={searchText}
              onSearchChange={setSearchText}
              level={currentLogLevel}
              onLevelChange={handleLogLevelChange}
              isPaused={isPaused}
              onTogglePause={() => setIsPaused(!isPaused)}
              onExport={exportLogs}
              onClear={clearLogs}
              hasLogs={filteredLogs.length > 0}
              audienceFilter={audienceFilter}
              onAudienceFilterChange={setAudienceFilter}
              traceFilter={traceFilter}
              onClearTraceFilter={() => setTraceFilter(null)}
            />

            {loadError !== null && (
              <div
                role="alert"
                className="flex items-center justify-between gap-2 rounded-xl border border-danger-rim bg-danger-soft px-4 py-2"
              >
                <span
                  className="text-caption text-danger truncate"
                  title={`${t('settings.logs.loadErrorPrefix')} ${loadError}`}
                >
                  {t('settings.logs.loadErrorPrefix')} {loadError}
                </span>
                <button
                  type="button"
                  onClick={retryLoad}
                  className="shrink-0 rounded-lg border border-danger-rim bg-danger-soft px-3 py-1 text-micro font-bold uppercase tracking-wider text-danger hover:bg-danger hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger transition duration-150"
                >
                  {t('settings.logs.retry')}
                </button>
              </div>
            )}

            <div className="relative rounded-2xl border border-border-subtle bg-surface-raise/60 shadow-2xl overflow-hidden backdrop-blur-md">
              <div className="flex items-center justify-between bg-surface-raise/40 px-4 py-2.5 border-b border-border-subtle">
                <div className="flex items-center gap-2">
                  <div className="flex gap-1.5" aria-hidden="true">
                    <div className="h-2.5 w-2.5 rounded-full border border-danger-rim bg-danger-soft" />
                    <div className="h-2.5 w-2.5 rounded-full border border-warn-rim bg-warn-soft" />
                    <div className="h-2.5 w-2.5 rounded-full border border-ok-rim bg-ok-soft" />
                  </div>
                  <span className="text-micro tracking-widest text-text-muted uppercase ml-2 select-none">
                    {t('settings.logs.systemOutput')}
                  </span>
                </div>
                {isPaused && (
                  <span className="text-micro font-bold bg-warn-soft border border-warn-rim text-warn rounded px-1.5 py-0.5 animate-pulse uppercase tracking-wider">
                    {t('settings.logs.streamingPaused')}
                  </span>
                )}
              </div>

              <div
                ref={containerRef}
                onScroll={handleScroll}
                className="h-[clamp(22rem,calc(100vh_-_26rem),56rem)] overflow-y-auto px-4 py-3 font-mono text-caption leading-relaxed scrollbar-thin scrollbar-track-transparent scrollbar-thumb-border-subtle select-text selection:bg-primary/30 selection:text-text-main"
              >
                <LazyList
                  items={filteredLogs}
                  itemKey={({ seq }) => seq}
                  estimatedItemHeight={22}
                  role="log"
                  ariaLabel={t('settings.logs.historyAriaLabel')}
                  className="space-y-1.5"
                  renderItem={({ item }) => <LogLine log={item} onPickTrace={setTraceFilter} />}
                  empty={
                    <div className="flex h-full flex-col items-center justify-center gap-2 text-text-subtle select-none">
                      <Terminal size={20} className="opacity-25" />
                      {audienceFilter === 'human' ? (
                        <>
                          <p className="font-sans text-xs">{t('settings.logs.emptyHuman')}</p>
                          <button
                            type="button"
                            onClick={() => setAudienceFilter('ai')}
                            className="rounded-lg border border-border-subtle px-2 py-1 font-sans text-caption text-text-muted transition-colors hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                          >
                            {t('settings.logs.viewFullDiagnostic')}
                          </button>
                        </>
                      ) : (
                        <p className="font-sans text-xs">{t('settings.logs.emptyFiltered')}</p>
                      )}
                    </div>
                  }
                />
              </div>

              <div className="flex items-center justify-between px-4 py-2 border-t border-border-subtle bg-surface-raise/40 text-caption text-text-muted select-none">
                <div className="flex items-center gap-1.5">
                  <div
                    aria-hidden="true"
                    className={`h-1.5 w-1.5 rounded-full ${autoScroll ? 'bg-info animate-pulse' : 'bg-text-subtle'}`}
                  />
                  <span>
                    {t('settings.logs.autoScroll.label')}:{' '}
                    {autoScroll ? t('settings.logs.autoScroll.active') : t('settings.logs.autoScroll.paused')}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <div role="status" aria-live="polite">
                    {filteredLogs.length === 1
                      ? t('settings.logs.showingSingular')
                      : t('settings.logs.showingPlural').replace('{{count}}', String(filteredLogs.length))}
                  </div>
                  {isTruncated && (
                    <span
                      className="text-warn"
                      title={t('settings.logs.truncatedTitle').replace('{{total}}', String(totalCaptured))}
                    >
                      {t('settings.logs.truncatedNote').replace('{{max}}', String(MAX_LOGS))}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </GlassSection>
    </div>
  )
}
