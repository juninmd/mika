import type { FeatureSettingsDeclaration, PluginUiCatalog } from '@mika/sdk/contracts'
import { getLocale, subscribeLocale, type TranslationDict, t } from '@mika/sdk/i18n'
import { useCallback, useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { readLocalStorage, writeLocalStorage } from '../lib/safe-local-storage'
import type { ToolCatalogItem } from '../lib/shell-api'
import { useTimeoutRef } from './useTransientFlag'

export type SettingsFetchResult = [
  PromiseSettledResult<{ declarations: FeatureSettingsDeclaration[] }>,
  PromiseSettledResult<{ config: Record<string, unknown> }>,
  PromiseSettledResult<{ status: string; settingsTabs: number; tools: number }>,
  PromiseSettledResult<{ tools: ToolCatalogItem[] }>,
  PromiseSettledResult<PluginUiCatalog>,
]

export const EMPTY_PLUGIN_UI: PluginUiCatalog = { views: [], settingsPanels: [] }

export function settledValue<T>(result: PromiseSettledResult<T>, label: string, fallback: T): T {
  if (result.status === 'fulfilled') return result.value
  rendererLog.warn(`[Settings] Failed to load ${label}:`, result.reason)
  return fallback
}

/** Built in the active locale on every call: a module-level constant would freeze the first language. */
export function overviewTab(): FeatureSettingsDeclaration {
  return {
    tabId: 'overview',
    tabLabel: t('settings.tab.overview'),
    tabDescription: t('settings.tab.overview.description'),
    tabIcon: 'Spark',
    tabGroup: 'General',
    heroNote: t('settings.tab.overview.heroNote'),
    accentColor: 'teal',
    groups: [],
  }
}

export function modulesTab(): FeatureSettingsDeclaration {
  return {
    tabId: 'modules',
    tabLabel: t('settings.tab.modules'),
    tabDescription: t('settings.tab.modules.description'),
    tabIcon: 'MonitorCog',
    tabGroup: 'General',
    heroNote: t('settings.tab.modules.heroNote'),
    accentColor: 'orange',
    groups: [],
  }
}

// Human field label, or null if the path isn't a screen field — keeps the status bar from
// announcing internal identifiers for runtime changes the user never made.
export function fieldLabelResolver(declarations: FeatureSettingsDeclaration[]): (path: string) => string | null {
  const labels = new Map<string, string>()
  for (const declaration of declarations) {
    for (const group of declaration.groups) {
      for (const field of group.fields) labels.set(field.configPath, field.label)
    }
  }
  return (path) => labels.get(path) ?? null
}

export type StatusMessage = { key: keyof TranslationDict; detail?: string }

export const READY_STATUS: StatusMessage = { key: 'settings.status.ready' }

export const ACTIVE_TAB_STORAGE_KEY = 'mika.settings.activeTabId'
export const SIDEBAR_OPEN_STORAGE_KEY = 'mika.settings.sidebarOpen'

export function readStoredActiveTabId(): string | null {
  return readLocalStorage(ACTIVE_TAB_STORAGE_KEY)
}

export function readStoredSidebarOpen(): boolean {
  const raw = readLocalStorage(SIDEBAR_OPEN_STORAGE_KEY)
  return raw === null ? true : raw === 'true'
}

export function useLabelForPath(declarations: FeatureSettingsDeclaration[]) {
  // Ref keeps labelForPath stable; without it, each declarations reload would resubscribe onConfigChanged.
  const labelResolver = useMemo(() => fieldLabelResolver(declarations), [declarations])
  const labelForPathRef = useRef(labelResolver)
  useEffect(() => {
    labelForPathRef.current = labelResolver
  }, [labelResolver])
  const labelForPath = useCallback((path: string) => labelForPathRef.current(path), [])
  return labelForPath
}

export function useSettingsStatus(
  statusMessage: StatusMessage,
  setStatusMessage: (update: (prev: StatusMessage) => StatusMessage) => void,
) {
  // Rendered at display time, not decision time, or it would stay in the old locale after a switch.
  const locale = useSyncExternalStore(subscribeLocale, getLocale, getLocale)
  const status = useMemo(() => {
    const label = t(statusMessage.key, locale)
    return statusMessage.detail ? `${label}: ${statusMessage.detail}` : label
  }, [statusMessage, locale])

  const updateStatus = useCallback(
    (next: StatusMessage) => {
      setStatusMessage((prev) => (prev.key === next.key && prev.detail === next.detail ? prev : next))
    },
    [setStatusMessage],
  )

  // `set` is stable; the object useTimeoutRef returns is not — depending on it would resubscribe every render.
  const { set: scheduleStatusReset } = useTimeoutRef()
  const resetStatusSoon = useCallback(
    (delayMs: number) => scheduleStatusReset(() => updateStatus(READY_STATUS), delayMs),
    [scheduleStatusReset, updateStatus],
  )
  return { status, updateStatus, resetStatusSoon }
}

export function usePersistedSettingsUi(activeTabId: string | null, sidebarOpen: boolean) {
  useEffect(() => {
    if (!activeTabId) return
    writeLocalStorage(ACTIVE_TAB_STORAGE_KEY, activeTabId)
  }, [activeTabId])

  useEffect(() => {
    writeLocalStorage(SIDEBAR_OPEN_STORAGE_KEY, String(sidebarOpen))
  }, [sidebarOpen])
}
