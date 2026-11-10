import type { ElectronShellApi } from '../../../shared/electron-api-types'
import { createPluginRpcRequest, unwrapPluginRpcResponse } from '../../../shared/plugin-rpc-contract'

export async function callPluginRpc<T = unknown>(
  eAPI: ElectronShellApi,
  pluginId: string,
  method: string,
  args?: unknown,
): Promise<T> {
  if (!eAPI.shell.pluginRpc) throw new Error(`Plugin RPC is unavailable for ${pluginId}`)
  const response = await eAPI.shell.pluginRpc(createPluginRpcRequest(pluginId, method, args))
  return unwrapPluginRpcResponse<T>(response)
}

/**
 * `callPluginRpc` strips the transport envelope only. The gamer-companion handlers answer with the bare data
 * (`modules/gamer/rpc.rs`), while the TS-era ones — and person-catalog still — answer `{ success, data }`.
 * Readers of the former go through here so neither shape is read as a failure.
 */
export function unwrapRpcData<T>(value: unknown): T {
  if (value !== null && typeof value === 'object' && typeof (value as { success?: unknown }).success === 'boolean') {
    const envelope = value as { success: boolean; data?: T; error?: string }
    if (!envelope.success) throw new Error(envelope.error ?? 'plugin RPC failed')
    return envelope.data as T
  }
  return value as T
}
