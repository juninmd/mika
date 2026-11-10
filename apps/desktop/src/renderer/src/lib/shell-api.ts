import type { ElectronShellApi } from '../../../shared/electron-api-types'

export type { ToolCatalogItem } from '../../../shared/electron-api-types'

let _cachedApi: ElectronShellApi | null = null

export function getElectronApi(): ElectronShellApi {
  if (_cachedApi) return _cachedApi
  const { electronAPI } = window as unknown as { electronAPI?: ElectronShellApi }
  if (!electronAPI) {
    throw new Error('electronAPI bridge is unavailable')
  }
  _cachedApi = electronAPI
  return _cachedApi
}
