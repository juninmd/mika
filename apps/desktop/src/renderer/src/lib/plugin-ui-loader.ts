import type { RendererApi } from '@mika/sdk-ui'
import type { ElectronShellApi } from '../../../shared/electron-api-types'
import { callPluginRpc } from './plugin-rpc'

export type PluginUIConfig = {
  views?: Record<string, React.ComponentType>
  settingsPanels?: Record<string, React.ComponentType>
}

export function createPluginProviderApi(eAPI: ElectronShellApi, pluginId: string): RendererApi {
  return {
    openSettings: () => eAPI.shell.openSettings?.(),
    mika: {
      executeTool: (name: string, args: Record<string, unknown>) =>
        eAPI.runtime.executeTool?.(name, args) ?? Promise.reject(new Error('executeTool not available')),
      getSettingsConfig: () => eAPI.runtime.getSettingsConfig(),
      saveSetting: (path: string, value: unknown) => eAPI.runtime.saveSetting(path, value),
      sendChatMessage: (message: string) => eAPI.runtime.sendChatMessage(message),
    },
    rpc: {
      call: async <T = unknown>(method: string, args?: unknown) => callPluginRpc<T>(eAPI, pluginId, method, args),
    },
  }
}
