import type { ConfigChangeEvent, FeatureSettingsDeclaration, PluginUiCatalog } from '@mika/sdk/contracts'
import { ConfigKey } from '@mika/sdk/contracts'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { BUILT_IN_TAB_IDS } from '../components/settings/MikaNavRail'
import { filterDeclarations, getByPath, groupDeclarations, setByPath } from '../components/settings/settings-utils'
import { rendererLog } from '../lib/renderer-logger'
import { isSensitivePath } from '../lib/sensitive-paths'
import { useSettingsNavigation } from '../lib/settings-navigation'
import type { ToolCatalogItem } from '../lib/shell-api'
import { getElectronApi } from '../lib/shell-api'
import {
  EMPTY_PLUGIN_UI,
  modulesTab,
  overviewTab,
  READY_STATUS,
  readStoredActiveTabId,
  readStoredSidebarOpen,
  type SettingsFetchResult,
  type StatusMessage,
  settledValue,
  useLabelForPath,
  usePersistedSettingsUi,
  useSettingsStatus,
} from './settings-controller-helpers'
import { useI18n } from './useI18n'

export function useSettingsController() {
  const electronApi = getElectronApi()
  const { locale } = useI18n()
  const [declarations, setDeclarations] = useState<FeatureSettingsDeclaration[]>([])
  const [config, setConfig] = useState<Record<string, unknown>>({})
  const [health, setHealth] = useState<{
    status: string
    settingsTabs: number
    tools: number
    bootstrap?: { errors: string[] }
  } | null>(null)
  const [tools, setTools] = useState<ToolCatalogItem[]>([])
  const [pluginUi, setPluginUi] = useState<PluginUiCatalog>({ views: [], settingsPanels: [] })
  const [activeTabId, setActiveTab] = useState<string | null>(() => readStoredActiveTabId())
  const [loading, setLoading] = useState(true)
  const [sidebarOpen, setSidebarOpen] = useState(() => readStoredSidebarOpen())
  const [search, setSearch] = useState('')
  const [statusMessage, setStatusMessage] = useState<StatusMessage>(READY_STATUS)

  const labelForPath = useLabelForPath(declarations)
  const { status, updateStatus, resetStatusSoon } = useSettingsStatus(statusMessage, setStatusMessage)

  const fetchAll = useCallback(async () => {
    try {
      const [declarationsResult, configResult, healthResult, toolsResult, pluginUiResult] = (await Promise.allSettled([
        electronApi.runtime.getSettingsDeclarations(),
        electronApi.runtime.getSettingsConfig(),
        electronApi.runtime.getHealth(),
        electronApi.runtime.getTools(),
        electronApi.runtime.getPluginUi(),
      ])) as SettingsFetchResult

      setDeclarations(settledValue(declarationsResult, 'declarations', { declarations: [] }).declarations)
      setConfig(settledValue(configResult, 'config', { config: {} }).config)
      setHealth(settledValue(healthResult, 'health', null))
      setTools(settledValue(toolsResult, 'tools', { tools: [] }).tools)
      setPluginUi(settledValue(pluginUiResult, 'plugin UI catalog', EMPTY_PLUGIN_UI))
      setActiveTab((prev) => prev ?? 'overview')
    } finally {
      setLoading(false)
    }
  }, [
    electronApi.runtime.getPluginUi,
    electronApi.runtime.getTools,
    electronApi.runtime.getSettingsDeclarations,
    electronApi.runtime.getSettingsConfig,
    electronApi.runtime.getHealth,
  ])

  useEffect(() => {
    void fetchAll()
    const unsub = electronApi.runtime.onConfigChanged((event: ConfigChangeEvent) => {
      setConfig((prev) => {
        if (Object.is(getByPath(prev, event.path), event.value)) return prev
        return setByPath(prev, event.path, event.value)
      })
      const changedLabel = labelForPath(event.path)
      if (changedLabel) {
        updateStatus({ key: 'settings.status.changed', detail: changedLabel })
        resetStatusSoon(3000)
      }

      if (event.path === ConfigKey.AppLocale) {
        electronApi.runtime
          .getSettingsDeclarations()
          .then(({ declarations: nextDeclarations }) => setDeclarations(nextDeclarations))
          .catch((error) => rendererLog.warn('[Settings] Failed to reload declarations after locale change:', error))
      }
    })
    return () => {
      unsub()
    }
  }, [electronApi.runtime, fetchAll, resetStatusSoon, updateStatus, labelForPath])

  const saveSetting = useCallback(
    async (path: string, value: unknown) => {
      rendererLog.info('[Settings] Saving validated settings payload', { changedPaths: [path] })
      const label = labelForPath(path) ?? path.split('.').pop() ?? path
      updateStatus({ key: 'settings.status.saving', detail: label })
      try {
        const result = await electronApi.runtime.saveSetting(path, value)
        if (!result.success) {
          const errorMessage = 'error' in result && typeof result.error === 'string' ? result.error : null
          const message = errorMessage || label
          updateStatus({ key: 'settings.status.saveFailed', detail: message })
          throw new Error(message)
        }
        rendererLog.info('[Settings] config updated', {
          path,
          value: isSensitivePath(path) ? '<REDACTED>' : value,
          valueType: typeof value,
        })
        updateStatus({ key: 'settings.status.saved', detail: label })
        resetStatusSoon(2500)
      } catch (error) {
        const message = toErrorMessage(error)
        rendererLog.warn('[Settings] config update failed', { path, error: message })

        updateStatus({ key: 'settings.status.saveFailed', detail: label })
        resetStatusSoon(3200)
        throw error
      }
    },
    [electronApi.runtime, resetStatusSoon, updateStatus, labelForPath],
  )

  usePersistedSettingsUi(activeTabId, sidebarOpen)
  useSettingsNavigation(setActiveTab)

  // `locale` in the deps: the built-in tabs' copy is rebuilt when the language changes.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `locale` is a trigger-only dependency.
  const shellDeclarations = useMemo(() => [overviewTab(), modulesTab(), ...declarations], [declarations, locale])

  useEffect(() => {
    if (loading) return
    // Built-in tabs aren't in shellDeclarations; excluding them here avoids bouncing back to overview.
    if (activeTabId && !BUILT_IN_TAB_IDS.has(activeTabId) && !shellDeclarations.some((d) => d.tabId === activeTabId)) {
      setActiveTab('overview')
    }
  }, [loading, activeTabId, shellDeclarations])
  const filtered = useMemo(() => filterDeclarations(shellDeclarations, search), [shellDeclarations, search])
  const active = useMemo(
    () => shellDeclarations.find((d) => d.tabId === activeTabId) || null,
    [shellDeclarations, activeTabId],
  )
  const grouped = useMemo(() => groupDeclarations(filtered), [filtered])
  const activeSettingsPanels = useMemo(
    () => pluginUi.settingsPanels.filter((panel) => active?.pluginId && panel.pluginId === active.pluginId),
    [active?.pluginId, pluginUi.settingsPanels],
  )

  const ctrl = useMemo(
    () => ({
      loading,
      declarations,
      shellDeclarations,
      activeTabId,
      setActiveTab,
      search,
      setSearch,
      sidebarOpen,
      setSidebarOpen,
      status,
      config,
      health,
      tools,
      active,
      grouped,
      pluginUi,
      activeSettingsPanels,
      saveSetting,
    }),
    [
      loading,
      declarations,
      shellDeclarations,
      activeTabId,
      search,
      sidebarOpen,
      status,
      config,
      health,
      tools,
      active,
      grouped,
      pluginUi,
      activeSettingsPanels,
      saveSetting,
    ],
  )

  return ctrl
}
