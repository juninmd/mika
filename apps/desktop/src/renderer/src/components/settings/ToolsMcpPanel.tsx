import { Activity, ArrowRight, Check, PlugZap, Server, Wrench, X } from 'lucide-react'
import { AuroraField, EmptyState, GlassSection, InfoRow, PageHero, StatTile, StatusPill } from './ds'
import { ToolsCatalogResults } from './ToolsCatalogResults'
import { useToolsMcpPanel } from './use-tools-mcp-panel'

type ToolsMcpPanelProps = {
  config: Record<string, unknown>
  /** "Gerenciar servidores" leva à aba MCP, onde se adiciona/testa/edita cada servidor. */
  onSelectTab: (tabId: string) => void
}

export function ToolsMcpPanel({ config, onSelectTab }: ToolsMcpPanelProps) {
  const {
    t,
    tools,
    loading,
    loadError,
    mcpTools,
    search,
    setSearch,
    recent,
    setReloadToken,
    pluginLabels,
    deferredSearch,
    filtered,
    groups,
  } = useToolsMcpPanel()

  const mcpEnabled = Boolean((config.mcp as Record<string, unknown> | undefined)?.enabled)

  return (
    <div className="space-y-6">
      <PageHero
        icon={Wrench}
        title={t('settings.toolsMcp.header.title')}
        description={t('settings.toolsMcp.header.description')}
        pills={
          loading ? (
            <StatusPill variant="pending" label={t('settings.toolsMcp.recentSection.loadingRegistry')} dot />
          ) : (
            <>
              <StatusPill
                variant={tools.length === 0 ? 'warn' : 'ok'}
                label={
                  tools.length === 0
                    ? t('settings.toolsMcp.recentSection.noneRegistered')
                    : t('settings.toolsMcp.recentSection.activeCount').replace('{{count}}', String(tools.length))
                }
                dot
              />
              <StatusPill
                variant={mcpEnabled ? 'accent' : 'inactive'}
                label={mcpEnabled ? t('settings.toolsMcp.hero.mcpOn') : t('settings.toolsMcp.hero.mcpOff')}
              />
            </>
          )
        }
        aside={
          <>
            <StatTile label={t('settings.toolsMcp.hero.modules')} value={String(groups.length)} />
            <StatTile label={t('settings.toolsMcp.hero.mcpTools')} value={String(mcpTools.length)} />
          </>
        }
      />

      <GlassSection
        title={t('settings.toolsMcp.mcpSection.title')}
        icon={Server}
        description={t('settings.toolsMcp.mcpSection.description')}
      >
        {mcpTools.length > 0 ? (
          <div className="space-y-2">
            <InfoRow
              label={t('settings.toolsMcp.mcpSection.stateLabel')}
              value={t('settings.toolsMcp.mcpSection.stateValue', { count: mcpTools.length })}
            />
          </div>
        ) : (
          <div className="flex items-start gap-2.5 rounded-xl border border-warn-rim bg-warn-soft px-4 py-3 text-xs leading-relaxed text-warn">
            <PlugZap size={16} className="mt-px shrink-0" aria-hidden="true" />
            <span>
              {mcpEnabled
                ? t('settings.toolsMcp.mcpSection.enabledEmpty')
                : t('settings.toolsMcp.mcpSection.disabledHint')}
            </span>
          </div>
        )}
        <button
          type="button"
          onClick={() => onSelectTab('mcp')}
          className="mt-3 inline-flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise/60 px-3 py-1.5 text-caption font-bold text-text-main transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <Server size={13} aria-hidden="true" />
          {t('settings.toolsMcp.mcpSection.manage')}
          <ArrowRight size={13} aria-hidden="true" />
        </button>
      </GlassSection>

      <GlassSection
        title={t('settings.toolsMcp.registrySection.title').replace('{{count}}', String(tools.length))}
        icon={Wrench}
        description={t('settings.toolsMcp.registrySection.description')}
      >
        <div className="space-y-4">
          <AuroraField
            label={t('settings.toolsMcp.searchAriaLabel')}
            hideLabel
            type="search"
            value={search}
            onChange={setSearch}
            placeholder={t('settings.toolsMcp.searchPlaceholder')}
          />

          {!loading && deferredSearch.trim() && (
            <p role="status" className="text-caption text-text-subtle">
              {t(
                filtered.length === 1 ? 'settings.toolsMcp.resultCountSingular' : 'settings.toolsMcp.resultCountPlural',
              ).replace('{{count}}', String(filtered.length))}
            </p>
          )}

          <ToolsCatalogResults
            loading={loading}
            loadError={loadError}
            groups={groups}
            deferredSearch={deferredSearch}
            onRetry={() => setReloadToken((n) => n + 1)}
            pluginLabels={pluginLabels}
            t={t}
          />
        </div>
      </GlassSection>

      <GlassSection
        title={t('settings.toolsMcp.recentSection.title')}
        icon={Activity}
        description={t('settings.toolsMcp.recentSection.description')}
      >
        {recent.length === 0 ? (
          <EmptyState
            icon={Activity}
            title={t('settings.toolsMcp.recentSection.waiting')}
            hint={t('settings.toolsMcp.recentSection.waitingHint')}
          />
        ) : (
          <div className="space-y-1.5">
            {recent.map((entry) => (
              <div
                key={entry.uid}
                className="flex items-center gap-2.5"
                title={
                  entry.durationMs !== undefined ? `${entry.name} (${Math.round(entry.durationMs)}ms)` : entry.name
                }
              >
                <span
                  role="img"
                  className="flex shrink-0"
                  aria-label={
                    entry.success
                      ? t('settings.toolsMcp.recentSection.successAria').replace('{{name}}', entry.name)
                      : t('settings.toolsMcp.recentSection.failureAria').replace('{{name}}', entry.name)
                  }
                >
                  {entry.success ? (
                    <Check size={13} className="text-ok" aria-hidden="true" />
                  ) : (
                    <X size={13} className="text-danger" aria-hidden="true" />
                  )}
                </span>
                <code className="font-mono text-caption text-text-main">{entry.name}</code>
                {!entry.success && entry.error && (
                  <span className="truncate text-caption text-danger" title={entry.error}>
                    {entry.error}
                  </span>
                )}
                {entry.durationMs !== undefined && (
                  <span className="ml-auto text-caption tabular-nums text-text-subtle">
                    {Math.round(entry.durationMs)}ms
                  </span>
                )}
              </div>
            ))}
          </div>
        )}
      </GlassSection>
    </div>
  )
}
