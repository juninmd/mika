import type { PluginViewDeclaration } from '@mika/sdk/contracts'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { PluginProvider } from '@mika/sdk-ui'
import { type ComponentType, useEffect, useState } from 'react'
import { useI18n } from './hooks/useI18n'
import { useTheme } from './hooks/useTheme'
import { loadFeatureUi } from './lib/feature-ui-registry'
import { createPluginProviderApi } from './lib/plugin-ui-loader'
import { getElectronApi } from './lib/shell-api'

export function PluginViewApp() {
  useTheme()
  const { t } = useI18n()
  const [Component, setComponent] = useState<ComponentType | null>(null)
  const [declaration, setDeclaration] = useState<PluginViewDeclaration | null>(null)
  const [error, setError] = useState<string | null>(null)

  const eAPI = getElectronApi()

  useEffect(() => {
    let disposed = false
    const params = new URLSearchParams(window.location.search)
    const pluginId = params.get('pluginId')
    const viewId = params.get('viewId')
    if (!pluginId || !viewId) {
      setError(t('shell.pluginView.missingParamsError'))
      return
    }
    const pluginIdValue = pluginId
    const viewIdValue = viewId

    async function load() {
      const catalog = await eAPI.runtime.getPluginUi()
      const view = catalog.views.find((item) => item.pluginId === pluginIdValue && item.id === viewIdValue)
      if (!view) {
        throw new Error(
          t('shell.pluginView.viewNotFoundError')
            .replace('{{pluginId}}', pluginIdValue)
            .replace('{{viewId}}', viewIdValue),
        )
      }
      const ui = loadFeatureUi(pluginIdValue)
      if (!ui) throw new Error(t('shell.pluginView.noUiError').replace('{{pluginId}}', pluginIdValue))
      const uiConfig = await ui
      const View = uiConfig.views?.[view.component]
      if (!View) {
        throw new Error(
          t('shell.pluginView.componentNotFoundError')
            .replace('{{pluginId}}', pluginIdValue)
            .replace('{{component}}', view.component),
        )
      }
      if (!disposed) {
        setDeclaration(view)
        setComponent(() => View)
      }
    }

    void load().catch((err) => {
      if (!disposed) setError(toErrorMessage(err))
    })
    return () => {
      disposed = true
    }
  }, [eAPI, t])

  if (error) return <div className="min-h-screen bg-bg-deep p-6 text-danger">{error}</div>
  if (!Component || !declaration)
    return <div className="min-h-screen bg-bg-deep p-6 text-text-muted">{t('status.loading')}</div>

  return (
    <PluginProvider
      value={{
        pluginId: declaration.pluginId,
        api: createPluginProviderApi(eAPI, declaration.pluginId),
      }}
    >
      {/* Opaque OS window: without its own background the widget sits on the webview's white. */}
      <div className="min-h-screen bg-bg-deep text-text-main">
        <header className="border-b border-border-subtle bg-bg-deep px-6 py-4">
          <h1 className="text-lg font-bold text-text-main">{declaration.title}</h1>
          {declaration.description && <p className="mt-1 text-sm text-text-muted">{declaration.description}</p>}
        </header>
        <Component />
      </div>
    </PluginProvider>
  )
}
