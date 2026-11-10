import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { LayoutDashboard } from 'lucide-react'
import { useMemo } from 'react'
import { LlmLiveStatus } from '../../../settings/llm/llm-live-status'
import { useLlmStatus } from '../../../settings/llm/use-llm-status'
import { useI18n } from '../../hooks/useI18n'
import { THEMES } from '../../themes'
import { builtInTabLabel } from './built-in-tab-labels'
import { PageHero, StatTile } from './ds'
import { hasReachableTab, MIKA_NAV_SECTIONS } from './MikaNavRail'
import { PerformanceProfileCard } from './PerformanceProfileCard'
import { configStr, getByPath, getResolvedModel, getToggleableDeclarations, truncate } from './settings-utils'
import { WindowModeCard } from './WindowModeCard'

type SettingsOverviewDeckProps = {
  declarations: FeatureSettingsDeclaration[]
  config: Record<string, unknown>
  health: { status: string; settingsTabs: number; tools: number } | null
  onSelectTab: (tabId: string) => void
  onSave: (path: string, value: string) => Promise<void>
}

export function SettingsOverviewDeck({ declarations, config, health, onSelectTab, onSave }: SettingsOverviewDeckProps) {
  const { t, locale } = useI18n()
  const toggleable = getToggleableDeclarations(declarations)
  // biome-ignore lint/correctness/useExhaustiveDependencies: `locale` re-resolves the built-in tab labels (read through the module-level `t`).
  const sections = useMemo(() => {
    const labelOf = (tabId: string) =>
      builtInTabLabel(tabId) ?? declarations.find((d) => d.tabId === tabId)?.tabLabel ?? null
    return MIKA_NAV_SECTIONS.filter(
      (section) => section.id !== 'sistema' && hasReachableTab(section, declarations),
    ).map((section) => {
      const tabs = section.tabIds.flatMap((tabId) => {
        const label = labelOf(tabId)
        return label ? [{ tabId, label }] : []
      })
      return { ...section, tabs }
    })
  }, [declarations, locale])
  const enabledCount = toggleable.filter((tab) => Boolean(getByPath(config, tab.enabledConfigPath ?? ''))).length
  const currentProvider = configStr(config, 'llm.provider', 'google')
  const currentModel = getResolvedModel(config, currentProvider)
  const currentThemeId = configStr(config, 'character.theme', THEMES[0].id)
  const currentTheme = THEMES.find((theme) => theme.id === currentThemeId) ?? THEMES[0]
  const { status: llmStatus, loading: llmLoading, refresh: refreshLlm } = useLlmStatus()

  return (
    <div className="space-y-6">
      <PageHero
        headingId="deck-hero-title"
        eyebrow={t('settings.overview.eyebrow')}
        title={t('settings.overview.title')}
        description={t('settings.overview.description')}
        icon={LayoutDashboard}
        aside={
          <>
            <StatTile
              label={t('settings.overview.llmLabel')}
              value={truncate(currentProvider, 14) + (currentModel ? ` · ${truncate(currentModel, 14)}` : '')}
              hint={t('settings.overview.llmHint')}
              tone={llmStatus ? (llmStatus.ready ? 'ok' : 'warn') : 'neutral'}
              onClick={() => onSelectTab('llm')}
            />
            <StatTile
              label={t('settings.overview.modulesLabel')}
              value={`${enabledCount} / ${toggleable.length}`}
              hint={t('settings.overview.modulesHint')}
              onClick={() => onSelectTab('modules')}
            />
            <StatTile
              label={t('settings.overview.toolsLabel')}
              value={`${health?.tools ?? 0}`}
              hint={t('settings.overview.toolsHint')}
              onClick={() => onSelectTab('tools-mcp')}
            />
            <StatTile
              label={t('settings.overview.themeLabel')}
              value={currentTheme.name}
              hint={t('settings.overview.themeHint')}
              onClick={() => onSelectTab('character')}
            />
          </>
        }
      />
      <div className="rounded-xl border border-border-subtle bg-glass p-4">
        <LlmLiveStatus status={llmStatus} loading={llmLoading} onRefresh={refreshLlm} compact />
      </div>
      <PerformanceProfileCard config={config} onSave={onSave} />
      <WindowModeCard />
      <nav aria-label={t('settings.overview.shortcutsAriaLabel')} className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {sections.map(({ id, labelKey, icon: Icon, tabs }) => (
          <button
            key={id}
            type="button"
            onClick={() => tabs[0] && onSelectTab(tabs[0].tabId)}
            className="group flex items-start gap-3 rounded-xl border border-border-subtle bg-glass p-4 text-left transition-colors hover:border-border-strong hover:bg-surface-raise"
          >
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-muted transition-colors group-hover:text-accent-text">
              <Icon size={17} />
            </span>
            <span className="min-w-0">
              <span className="block text-sm font-semibold text-text-main">{t(labelKey)}</span>
              <span className="mt-0.5 block text-xs leading-relaxed text-text-muted">
                {tabs.map((tab) => tab.label).join(' · ')}
              </span>
            </span>
          </button>
        ))}
      </nav>
    </div>
  )
}
