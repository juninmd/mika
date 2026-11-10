import type { ChatRuntimeApi } from './electron-api-chat-types'
import type { EventsRuntimeApi } from './electron-api-events-types'
import type { SettingsRuntimeApi } from './electron-api-settings-types'
import type { SystemRuntimeApi } from './electron-api-system-types'
import type { PluginRpcRequest, PluginRpcResponse } from './plugin-rpc-contract'

export type { ChatRuntimeApi, ToolCatalogItem } from './electron-api-chat-types'
export type { EventsRuntimeApi } from './electron-api-events-types'
export type { PromptSummaryPayload, SettingsRuntimeApi } from './electron-api-settings-types'
export type { RendererLogEntry, SystemRuntimeApi } from './electron-api-system-types'

export type MikaRuntimeApi = ChatRuntimeApi & SettingsRuntimeApi & EventsRuntimeApi & SystemRuntimeApi

export type ElectronShellNamespace = {
  openSettings: () => void
  window?: {
    close: () => void
    toggleAlwaysOnTop: () => Promise<boolean>
    /** Flips fullscreen on the avatar window (persisted as `app.fullscreen`); resolves to the new state. */
    toggleFullscreen?: () => Promise<boolean>
    isFullscreen?: () => Promise<boolean>
    onFullscreenChange?: (callback: (payload: { fullscreen: boolean }) => void) => () => void
    setIgnoreMouseEvents: (ignore: boolean) => void
    moveBy: (deltaX: number, deltaY: number) => void
    startDrag?: () => void
    onMouseMove: (
      callback: (pos: { x: number; y: number; insideX: number; insideY: number; isInside: boolean }) => void,
    ) => () => void
  }
  autoLaunch?: {
    get: () => Promise<boolean>
    set: (val: boolean) => Promise<void>
  }
  shortcuts?: {
    onToggleInput: (callback: () => void) => () => void
    onToggleMic: (callback: () => void) => () => void
    /** F8 held (`true`) / released (`false`), from `shortcuts.rs` — no focus needed. */
    onPushToTalk: (callback: (pressed: boolean) => void) => () => void
  }
  pluginViews?: {
    open: (payload: { pluginId: string; viewId: string }) => Promise<void>
  }
  pluginRpc: (request: PluginRpcRequest) => Promise<PluginRpcResponse>

  system?: {
    preventSleep: (enabled: boolean) => Promise<{ ok: boolean }>
    /** Opens an allowlisted https link (key pages, help) in the default browser. */
    openExternal?: (url: string) => Promise<void>
    /** Opens the app data or logs folder in the OS file manager; resolves to its real path. */
    revealPath?: (kind: 'data' | 'logs') => Promise<{ path: string }>
    /** Quits the whole app (tray "Quit" sequence), not just the calling window. */
    quit?: () => Promise<void>
  }
}

export type ElectronShellApi = {
  platform: string
  versions: {
    node: string
    chrome: string
    electron: string
  }
  runtime: MikaRuntimeApi
  shell: ElectronShellNamespace
}
