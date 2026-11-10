import type { SettingsPanelDeclaration } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { type ComponentType, memo, useEffect, useMemo, useState } from 'react'
import type { ElectronShellApi } from '../../../../shared/electron-api-types'
import { useI18n } from '../../hooks/useI18n'
import { loadFeatureUi } from '../../lib/feature-ui-registry'
import { createPluginProviderApi } from '../../lib/plugin-ui-loader'
import { rendererLog } from '../../lib/renderer-logger'
import { getElectronApi } from '../../lib/shell-api'
import { PluginSlot } from '../PluginSlot'

type LoadedSettingsPanel =
  | { kind: 'component'; panel: SettingsPanelDeclaration; Component: ComponentType }
  | { kind: 'error'; panel: SettingsPanelDeclaration; message: string }

async function loadSettingsPanel(
  panel: SettingsPanelDeclaration,
  t: ReturnType<typeof useI18n>['t'],
): Promise<LoadedSettingsPanel | null> {
  try {
    const ui = loadFeatureUi(panel.pluginId)
    if (!ui) return null
    const config = await ui
    const Component = config.settingsPanels?.[panel.component]
    if (!Component) {
      return {
        kind: 'error',
        panel,
        message: t('settings.pluginPanels.notExported').replace('{{component}}', panel.component),
      }
    }
    return { kind: 'component', panel, Component }
  } catch (reason) {
    return {
      kind: 'error',
      panel,
      message: toDisplayError(reason, t('settings.pluginPanels.loadFailed')),
    }
  }
}

function LoadedPanelSlot({
  panel,
  Component,
  eAPI,
}: {
  panel: SettingsPanelDeclaration
  Component: ComponentType
  eAPI: ElectronShellApi
}) {
  const api = useMemo(() => createPluginProviderApi(eAPI, panel.pluginId), [eAPI, panel.pluginId])
  const context = useMemo(() => ({ pluginId: panel.pluginId, api }), [api, panel.pluginId])

  return (
    <PluginSlot name={panel.component} pluginId={panel.pluginId} context={context}>
      <Component />
    </PluginSlot>
  )
}

const LoadedPanelSlotMemo = memo(LoadedPanelSlot)

export function PluginSettingsPanels({ panels }: { panels: SettingsPanelDeclaration[] }) {
  const { t } = useI18n()
  const [loadedPanels, setLoadedPanels] = useState<LoadedSettingsPanel[]>([])
  const [loading, setLoading] = useState(panels.length > 0)
  const eAPI = getElectronApi()

  useEffect(() => {
    let disposed = false
    async function loadPanels() {
      if (panels.length === 0) {
        setLoadedPanels([])
        setLoading(false)
        return
      }
      setLoading(true)
      const loaded = await Promise.all(panels.map((panel) => loadSettingsPanel(panel, t)))
      if (!disposed) setLoadedPanels(loaded.filter((item): item is LoadedSettingsPanel => Boolean(item)))
      if (!disposed) setLoading(false)
    }

    void loadPanels().catch((reason) => {
      rendererLog.warn('[Settings] Failed to load plugin settings panels:', reason)
      if (!disposed) setLoading(false)
    })
    return () => {
      disposed = true
    }
  }, [panels, t])

  if (panels.length === 0) return null

  if (loading) {
    return (
      <div
        role="status"
        aria-live="polite"
        className="animate-pulse rounded-lg border border-border-subtle bg-surface-raise px-4 py-6 text-center text-xs font-bold uppercase tracking-widest text-text-muted"
      >
        {t('settings.pluginPanels.loading')}
      </div>
    )
  }

  if (loadedPanels.length === 0) {
    return (
      <div
        role="status"
        className="rounded-lg border border-dashed border-border-subtle bg-surface-raise/30 px-4 py-6 text-center text-xs font-bold uppercase tracking-widest text-text-muted"
      >
        {t('settings.pluginPanels.none')}
      </div>
    )
  }

  return (
    <>
      {loadedPanels.map((item) => {
        if (item.kind === 'error') {
          return (
            <div
              key={item.panel.id}
              role="alert"
              className="rounded-lg border border-danger-rim bg-danger-soft px-4 py-3 text-xs text-danger"
            >
              {item.panel.pluginId}: {item.message}
            </div>
          )
        }

        const { panel, Component } = item
        return <LoadedPanelSlotMemo key={panel.id} panel={panel} Component={Component} eAPI={eAPI} />
      })}
    </>
  )
}
