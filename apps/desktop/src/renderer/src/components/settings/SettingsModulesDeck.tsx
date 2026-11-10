import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { Boxes, SearchX } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { EmptyState, PageHero, StatTile } from './ds'
import { ModulesToolbar } from './ModulesToolbar'
import { ModuleCard } from './SettingsModuleCard'
import { getBulkTogglePlan, getByPath } from './settings-utils'

type SettingsModulesDeckProps = {
  declarations: FeatureSettingsDeclaration[]
  activeTabId: string | null
  config: Record<string, unknown>
  toolCount?: number
  onSave: (path: string, value: unknown) => Promise<void>
  onSelectTab: (tabId: string) => void
}

export function SettingsModulesDeck({
  declarations,
  activeTabId,
  config,
  toolCount,
  onSave,
  onSelectTab,
}: SettingsModulesDeckProps) {
  const { t } = useI18n()
  const [savingPath, setSavingPath] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [bulkSaving, setBulkSaving] = useState(false)
  const [search, setSearch] = useState('')

  const modules = useMemo(
    () => declarations.filter((t) => t.tabId !== 'overview' && t.tabId !== 'modules'),
    [declarations],
  )
  const bulkTogglePlan = useMemo(() => getBulkTogglePlan(declarations, config), [config, declarations])

  const fieldCountByTab = useMemo(() => {
    const map = new Map<string, number>()
    for (const tab of declarations) {
      map.set(
        tab.tabId,
        tab.groups.reduce((sum, g) => sum + g.fields.length, 0),
      )
    }
    return map
  }, [declarations])

  const filteredModules = useMemo(() => {
    if (!search) return modules
    const term = search.toLowerCase()
    return modules.filter(
      (m) => m.tabLabel.toLowerCase().includes(term) || m.tabDescription?.toLowerCase().includes(term),
    )
  }, [modules, search])

  const toggle = useCallback(
    async (path: string, next: boolean) => {
      setSavingPath(path)
      setError(null)
      try {
        await onSave(path, next)
      } catch (e) {
        setError(toDisplayError(e, t('settings.modulesDeck.coreCommunicationError')))
      } finally {
        setSavingPath(null)
      }
    },
    [onSave, t],
  )

  const toggleAll = useCallback(
    async (nextValue: boolean) => {
      setBulkSaving(true)
      setError(null)
      const results = await Promise.allSettled(bulkTogglePlan.paths.map((p) => onSave(p, nextValue)))
      const failed = results.filter((result) => result.status === 'rejected').length
      if (failed > 0) {
        const key = failed === 1 ? 'settings.modulesDeck.bulkSaveFailedOne' : 'settings.modulesDeck.bulkSaveFailedMany'
        setError(t(key).replace('{{count}}', String(failed)))
      }
      setBulkSaving(false)
    },
    [bulkTogglePlan, onSave, t],
  )

  const handleSelectTab = useCallback((tabId: string) => onSelectTab(tabId), [onSelectTab])
  // O card fala em tabId; o que se grava é o caminho de config que liga o módulo. Passar o tabId
  // direto gravava em "brain"/"voice" — rota recusada pelo IPC, e o switch voltava sozinho.
  const handleToggleModule = useCallback(
    (tabId: string, next: boolean) => {
      const path = modules.find((m) => m.tabId === tabId)?.enabledConfigPath
      if (path) void toggle(path, next)
    },
    [modules, toggle],
  )

  const renderModuleCard = (tab: FeatureSettingsDeclaration, index: number) => {
    const path = tab.enabledConfigPath
    const isEnabled = path ? Boolean(getByPath(config, path)) : true
    return (
      <ModuleCard
        key={tab.tabId}
        active={activeTabId === tab.tabId}
        enabled={isEnabled}
        fieldCount={fieldCountByTab.get(tab.tabId) ?? 0}
        index={index}
        saving={savingPath === path}
        tab={tab}
        onSelect={handleSelectTab}
        onToggle={path ? handleToggleModule : undefined}
      />
    )
  }

  return (
    <div className="space-y-6">
      <PageHero
        headingId="deck-hero-title"
        eyebrow={t('settings.modulesDeck.eyebrow')}
        title={t('settings.modulesDeck.title')}
        description={t('settings.modulesDeck.description')}
        icon={Boxes}
        aside={
          <>
            <StatTile label={t('settings.modulesDeck.activeTools')} value={`${toolCount ?? 0}`} />
            <StatTile
              label={t('settings.modulesDeck.enabledModules')}
              value={`${bulkTogglePlan.enabledCount} / ${bulkTogglePlan.toggleableCount}`}
            />
          </>
        }
      >
        {error && (
          <div
            role="alert"
            className="mt-4 rounded-lg border border-danger-rim bg-danger-soft px-3.5 py-3 text-xs font-bold text-danger"
          >
            {error}
          </div>
        )}
      </PageHero>

      <section aria-labelledby="modules-heading">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-4">
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <h3 id="modules-heading" className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
              {t('settings.modulesDeck.modulesHeading')}
            </h3>
            <div className="h-px flex-1 bg-border-subtle" aria-hidden="true" />
          </div>

          <ModulesToolbar
            bulkSaving={bulkSaving}
            bulkTogglePlan={bulkTogglePlan}
            search={search}
            setSearch={setSearch}
            toggleAll={toggleAll}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {filteredModules.map((tab, i) => renderModuleCard(tab, i))}
          {filteredModules.length === 0 && (
            <EmptyState
              icon={SearchX}
              title={t('settings.modulesDeck.noneFound')}
              size="block"
              className="col-span-full rounded-xl border border-dashed border-border-subtle"
            />
          )}
        </div>
      </section>
    </div>
  )
}
