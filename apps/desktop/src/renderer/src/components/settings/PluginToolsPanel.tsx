import type { ToolDetail } from '@mika/sdk/contracts'
import { memo, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { Collapsible, SectionHeader } from './ds'
import { ToolExecutionHistory } from './ToolExecutionHistory'
import { ToolExecutionView } from './ToolExecutionView'

type PluginToolsPanelProps = {
  tools: ToolDetail[]
  pluginId?: string
}

export const PluginToolsPanel = memo(function PluginToolsPanel({ tools, pluginId }: PluginToolsPanelProps) {
  const { t } = useI18n()
  const [expandedTool, setExpandedTool] = useState<string | null>(null)
  const [showHistory, setShowHistory] = useState(false)

  if (tools.length === 0) return null

  return (
    <section
      aria-label={t('settings.pluginTools.sectionAriaLabel')}
      className="rounded-[1.5rem] border border-border-subtle bg-surface-raise shadow-sm p-4 md:p-5"
    >
      <SectionHeader
        title={t('settings.pluginTools.sectionAriaLabel')}
        headingClassName="text-base font-bold tracking-tight md:text-lg"
        actions={
          <>
            <span className="inline-flex items-center gap-1 rounded-md border border-info-rim bg-info-soft px-2 py-1 text-micro font-extrabold uppercase tracking-wider text-info">
              {t('settings.pluginTools.neuralToolsCount').replace('{{count}}', String(tools.length))}
            </span>
            <button
              type="button"
              onClick={() => setShowHistory((p) => !p)}
              aria-expanded={showHistory}
              className="rounded-md border border-border-subtle bg-surface-raise px-2.5 py-1 text-micro font-bold uppercase tracking-wider text-text-muted transition active:scale-95 hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <span aria-hidden="true">{showHistory ? '▴' : '▾'}</span> {t('shell.history.badge')}
            </button>
          </>
        }
        className="mb-4"
      />

      <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {tools.map((tool) => {
          const isExpanded = expandedTool === tool.name
          return (
            <div
              key={tool.name}
              className={`group rounded-lg border p-3 text-left transition duration-200 hover:border-info-rim hover:bg-info-soft ${
                isExpanded
                  ? 'border-info-rim bg-info-soft shadow-xs'
                  : 'border-border-subtle bg-surface-raise/20 hover:border-border-strong hover:bg-surface-raise'
              }`}
            >
              <button
                type="button"
                onClick={() => setExpandedTool(isExpanded ? null : tool.name)}
                aria-expanded={isExpanded}
                className="w-full text-left rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5 mb-1">
                      <span className="h-1.5 w-1.5 shrink-0 rounded-sm bg-info shadow-xs" aria-hidden="true" />
                      <span className="truncate text-caption font-black uppercase tracking-widest text-info">
                        {tool.name}
                      </span>
                    </div>
                    <p
                      className={`text-caption leading-relaxed text-text-muted transition ${isExpanded ? '' : 'line-clamp-2'}`}
                    >
                      {tool.description}
                    </p>
                  </div>
                  <span
                    className="shrink-0 text-caption font-bold text-text-subtle transition-transform duration-200 group-hover:text-text-muted"
                    aria-hidden="true"
                  >
                    {isExpanded ? '▴' : '▾'}
                  </span>
                </div>
              </button>

              <Collapsible open={isExpanded} fast>
                <ToolExecutionView tool={tool} pluginId={pluginId} />
              </Collapsible>
            </div>
          )
        })}
      </div>

      <Collapsible open={showHistory} fast>
        <div className="mt-4 border-t border-border-subtle pt-4">
          <ToolExecutionHistory pluginId={pluginId} />
        </div>
      </Collapsible>
    </section>
  )
})
