import {
  DESKTOP_EVENT_CHANNELS,
  DESKTOP_LOCAL_INVOKE_ROUTES,
  DESKTOP_RENDERER_EVENT_CHANNELS,
  DESKTOP_SHELL_CHANNELS,
} from '@mika/sdk/contracts'
import type { ElectronShellApi, MikaRuntimeApi } from '../../shared/electron-api-types'
import { MikaInvokeChannel } from '../../shared/mika-invoke-channels'
import {
  createPluginRpcFailure,
  PluginRpcErrorCode,
  pluginRpcRequestSchema,
  pluginRpcResponseSchema,
} from '../../shared/plugin-rpc-contract'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { sendRendererLog } from './mika-runtime-api'

export function createElectronShellApi(mika: MikaRuntimeApi): ElectronShellApi {
  return {
    platform: process.platform,
    versions: {
      // Stage C: sem Electron, `chrome`/`electron` não existem mais em `process.versions` (o
      // WebView do Tauri não é Chromium empacotado) — `@types/node` já os declara opcionais.
      node: process.versions.node,
      chrome: process.versions.chrome ?? '',
      electron: process.versions.electron ?? '',
    },
    runtime: mika,
    shell: {
      openSettings: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.OpenSettings),
      window: {
        close: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.WindowClose),
        toggleAlwaysOnTop: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.WindowToggleAlwaysOnTop) as Promise<boolean>,
        toggleFullscreen: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.WindowToggleFullscreen) as Promise<boolean>,
        isFullscreen: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.WindowGetFullscreen) as Promise<boolean>,
        onFullscreenChange: (callback: (payload: { fullscreen: boolean }) => void) => {
          const listener = (_event: unknown, payload: { fullscreen: boolean }) => callback(payload)
          ipcRenderer.on(DESKTOP_RENDERER_EVENT_CHANNELS.WindowFullscreen, listener)
          return () => ipcRenderer.off(DESKTOP_RENDERER_EVENT_CHANNELS.WindowFullscreen, listener)
        },
        setIgnoreMouseEvents: (ignore: boolean) => ipcRenderer.send(DESKTOP_SHELL_CHANNELS.WindowIgnoreMouse, ignore),
        moveBy: (deltaX: number, deltaY: number) =>
          ipcRenderer.send(DESKTOP_SHELL_CHANNELS.WindowMove, { deltaX, deltaY }),
        startDrag: () => ipcRenderer.send(DESKTOP_SHELL_CHANNELS.WindowStartDrag),
        onMouseMove: (
          callback: (pos: { x: number; y: number; insideX: number; insideY: number; isInside: boolean }) => void,
        ) => {
          const listener = (
            _event: unknown,
            pos: { x: number; y: number; insideX: number; insideY: number; isInside: boolean },
          ) => callback(pos)
          ipcRenderer.on(DESKTOP_RENDERER_EVENT_CHANNELS.MouseMove, listener)
          return () => ipcRenderer.off(DESKTOP_RENDERER_EVENT_CHANNELS.MouseMove, listener)
        },
      },
      autoLaunch: {
        get: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.AutoLaunchGet) as Promise<boolean>,
        set: (val: boolean) => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.AutoLaunchSet, val) as Promise<void>,
      },
      shortcuts: {
        onToggleInput: (callback: () => void) => {
          const listener = () => callback()
          ipcRenderer.on(DESKTOP_EVENT_CHANNELS.ShortcutToggleInput, listener)
          return () => ipcRenderer.off(DESKTOP_EVENT_CHANNELS.ShortcutToggleInput, listener)
        },
        onToggleMic: (callback: () => void) => {
          const listener = () => callback()
          ipcRenderer.on(DESKTOP_EVENT_CHANNELS.ShortcutToggleMic, listener)
          return () => ipcRenderer.off(DESKTOP_EVENT_CHANNELS.ShortcutToggleMic, listener)
        },
        onPushToTalk: (callback: (pressed: boolean) => void) => {
          const listener = (_event: unknown, payload: { pressed?: unknown }) => callback(payload?.pressed === true)
          ipcRenderer.on(DESKTOP_EVENT_CHANNELS.ShortcutPushToTalk, listener)
          return () => ipcRenderer.off(DESKTOP_EVENT_CHANNELS.ShortcutPushToTalk, listener)
        },
      },
      pluginViews: {
        open: (payload: { pluginId: string; viewId: string }) =>
          ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.PluginViewOpen, payload),
      },
      pluginRpc: async (request) => {
        const parsedRequest = pluginRpcRequestSchema.safeParse(request)
        if (!parsedRequest.success) {
          void sendRendererLog({
            level: 'warn',
            scope: 'preload',
            msg: 'invalid plugin RPC request',
            meta: { issueCount: parsedRequest.error.issues.length },
          })
          return createPluginRpcFailure(PluginRpcErrorCode.InvalidRequest, 'Invalid plugin RPC request')
        }

        const response = await ipcRenderer.invoke(MikaInvokeChannel.PluginRpc, parsedRequest.data)
        const parsedResponse = pluginRpcResponseSchema.safeParse(response)
        if (!parsedResponse.success) {
          void sendRendererLog({
            level: 'warn',
            scope: 'preload',
            msg: 'invalid plugin RPC response',
            meta: { issueCount: parsedResponse.error.issues.length },
          })
          return createPluginRpcFailure(PluginRpcErrorCode.HandlerFailed, 'Invalid plugin RPC response')
        }

        return parsedResponse.data
      },
      system: {
        preventSleep: (enabled: boolean) =>
          ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.systemPreventSleep.channel, { enabled }),
        openExternal: (url: string) => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.OpenExternal, { url }),
        revealPath: (kind: 'data' | 'logs') =>
          ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.RevealPath, { kind }) as Promise<{ path: string }>,
        quit: () => ipcRenderer.invoke(DESKTOP_SHELL_CHANNELS.AppQuit) as Promise<void>,
      },
    },
  }
}
