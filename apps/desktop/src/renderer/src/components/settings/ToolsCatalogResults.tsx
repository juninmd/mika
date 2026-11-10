import { AlertTriangle, Loader2, SearchX } from 'lucide-react'
import type { ToolCatalogItem } from '../../lib/shell-api'
import { BentoGrid, EmptyState, GlassCard, SectionLabel, StatusPill } from './ds'
import type { LabeledOption } from './settings-utils'
import { labelOf } from './settings-utils'
import type { TKey } from './tools-mcp-catalog'

type ToolsCatalogResultsProps = {
  loading: boolean
  loadError: boolean
  groups: Array<[string, ToolCatalogItem[]]>
  deferredSearch: string
  onRetry: () => void
  pluginLabels: LabeledOption[]
  t: (key: TKey) => string
}

export function ToolsCatalogResults({
  loading,
  loadError,
  groups,
  deferredSearch,
  onRetry,
  pluginLabels,
  t,
}: ToolsCatalogResultsProps) {
  if (loading) {
    return <EmptyState icon={Loader2} title={t('settings.toolsMcp.loading')} />
  }
  if (loadError) {
    return (
      <div role="alert">
        <EmptyState
          icon={AlertTriangle}
          title={t('settings.toolsMcp.loadError')}
          action={
            <button
              type="button"
              onClick={onRetry}
              className="rounded-lg border border-danger-rim bg-danger-soft px-3 py-1.5 text-caption font-semibold text-danger transition-colors hover:border-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('settings.toolsMcp.retry')}
            </button>
          }
        />
      </div>
    )
  }
  if (groups.length === 0) {
    const term = deferredSearch.trim()
    return (
      <EmptyState
        icon={SearchX}
        title={term ? t('settings.toolsMcp.emptyFiltered').replace('{{term}}', term) : t('settings.toolsMcp.emptyAll')}
        className="rounded-xl border border-dashed border-border-subtle"
      />
    )
  }
  return (
    <BentoGrid cols="auto">
      {groups.map(([pluginId, items]) => (
        <GlassCard key={pluginId} span={items.length > 2 ? 'full' : undefined}>
          <SectionLabel count={items.length} className="mb-2">
            {labelOf(pluginLabels, pluginId)}
          </SectionLabel>
          <ul className="divide-y divide-border-subtle">
            {items.map((tool) => (
              <li key={tool.name} className="space-y-1 py-2 first:pt-0 last:pb-0">
                <div className="flex min-w-0 flex-wrap items-center gap-1.5">
                  <code className="truncate font-mono text-xs font-semibold text-accent-text">{tool.name}</code>
                  {tool.requiresWindow && <StatusPill variant="neutral" label={t('settings.toolsMcp.windowBadge')} />}
                </div>
                {tool.description && (
                  // Descriptions are written for the LLM and run to a paragraph; the full text stays in the tooltip.
                  <p title={tool.description} className="line-clamp-2 text-xs leading-relaxed text-text-muted">
                    {tool.description}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </GlassCard>
      ))}
    </BentoGrid>
  )
}
