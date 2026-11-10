import { rendererLog } from './renderer-logger'

/** `localStorage` pode lançar em modo privado/storage bloqueado — nunca deixe isso derrubar o chamador. */
export function readLocalStorage(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch (err) {
    rendererLog.warn('[SafeLocalStorage] Failed to read key', { key, err })
    return null
  }
}

export function writeLocalStorage(key: string, value: string): boolean {
  try {
    localStorage.setItem(key, value)
    return true
  } catch (err) {
    rendererLog.warn('[SafeLocalStorage] Failed to write key', { key, err })
    return false
  }
}
