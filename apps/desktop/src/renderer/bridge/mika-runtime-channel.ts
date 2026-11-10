import { DESKTOP_INVOKE_CHANNELS } from '@mika/sdk/contracts'
import type { RendererLogEntry } from '../../shared/electron-api-types'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'

export function sendRendererLog(entry: RendererLogEntry): Promise<boolean> {
  return (ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.RendererLog, entry) as Promise<{ ok?: boolean }>).then(
    (result) => result?.ok === true,
    () => false,
  )
}

export function onChannel<T>(channel: string, callback: (payload: T) => void): () => void {
  const listener = (_event: unknown, payload: T) => callback(payload)
  ipcRenderer.on(channel, listener)
  return () => ipcRenderer.off(channel, listener)
}
