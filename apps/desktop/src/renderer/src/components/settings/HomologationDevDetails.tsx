import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { ChevronDown, Code2 } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { Collapsible, GlassCard, StatusPill } from './ds'
import { FEATURE_COMPONENTS } from './feature-components'

export function hasDedicatedPanel(tab: FeatureSettingsDeclaration): boolean {
  return Boolean(
    (tab.component && FEATURE_COMPONENTS[tab.component]) ||
      (tab.groups.length > 0 && tab.groups.every((g) => g.component && FEATURE_COMPONENTS[g.component])),
  )
}

/**
 * UI-compliance bookkeeping (dedicated panel vs. schema form) — a developer metric, not something the
 * person using Mika acts on, so it only exists in dev builds and starts collapsed.
 */
export function HomologationDevDetails({ declarations }: { declarations: FeatureSettingsDeclaration[] }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const total = declarations.length
  const withPanel = declarations.filter(hasDedicatedPanel)
  const fallbacks = declarations.filter((d) => !hasDedicatedPanel(d))
  const compliance = total > 0 ? Math.round((withPanel.length / total) * 100) : 0

  return (
    <GlassCard span="full" hover={false}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-controls="diagnostics-dev-details"
        className="flex w-full items-center gap-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded-lg"
      >
        <Code2 size={14} className="text-text-muted" aria-hidden="true" />
        <span className="text-caption font-bold uppercase tracking-[0.12em] text-text-muted">
          {t('settings.diagnostics.devTitle')}
        </span>
        <StatusPill
          variant={fallbacks.length === 0 ? 'ok' : 'warn'}
          label={`${t('settings.homologation.panel.compliance')} ${compliance}%`}
        />
        <ChevronDown
          size={14}
          className={`ml-auto text-text-muted transition-transform ${open ? 'rotate-180' : ''}`}
          aria-hidden="true"
        />
      </button>
      <Collapsible open={open} id="diagnostics-dev-details">
        <div className="mt-3 grid gap-3 text-caption sm:grid-cols-3">
          <p className="text-text-muted">
            {t('settings.homologation.panel.totalTabs')}: <span className="font-mono text-text-main">{total}</span>
          </p>
          <p className="text-text-muted">
            {t('settings.homologation.panel.withDedicatedPanel')}:{' '}
            <span className="font-mono text-text-main">{withPanel.length}</span>
          </p>
          <p className="text-text-muted">
            {t('settings.homologation.panel.activeFallbacks')}:{' '}
            <span className={`font-mono ${fallbacks.length > 0 ? 'text-warn' : 'text-text-main'}`}>
              {fallbacks.length}
            </span>
          </p>
        </div>
        {fallbacks.length > 0 && (
          <p className="mt-2 text-caption text-text-muted">
            {t('settings.homologation.featureRow.schemaForm')}:{' '}
            <span className="font-mono text-warn">{fallbacks.map((d) => d.tabId).join(', ')}</span>
          </p>
        )}
        <p className="mt-2 text-caption leading-relaxed text-text-subtle">
          {t('settings.homologation.panel.criteriaDescription')}
        </p>
      </Collapsible>
    </GlassCard>
  )
}
