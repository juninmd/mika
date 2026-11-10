import { DESKTOP_SEND_CHANNELS } from '@mika/sdk/contracts'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ipcRendererInvoke, ipcRendererOff, ipcRendererOn, ipcRendererSend, setMaxListeners } from './ipc-fakes'

vi.mock('../../../shared/tauri-renderer-runtime', () => ({
  ipcRenderer: {
    invoke: (...args: unknown[]) => ipcRendererInvoke(...args),
    send: (...args: unknown[]) => ipcRendererSend(...args),
    on: (...args: unknown[]) => ipcRendererOn(...args),
    off: (...args: unknown[]) => ipcRendererOff(...args),
    setMaxListeners: (...args: unknown[]) => setMaxListeners(...args),
  },
}))

describe('mika-runtime-api bulk coverage', () => {
  let mod: typeof import('../mika-runtime-api')

  beforeEach(async () => {
    vi.clearAllMocks()
    vi.resetModules()
    mod = await import('../mika-runtime-api')
  })

  it('every zero-argument getter invokes exactly one ipc channel', () => {
    const api = mod.createMikaRuntimeApi()
    const getters = [
      api.getChatHistory,
      api.clearChatHistory,
      api.cancelChatMessage,
      api.getSettingsDeclarations,
      api.getSettingsConfig,
      api.getUiPrefs,
      api.getPrompts,
      api.getTools,
      api.getPluginUi,
      api.getPowerState,
      api.checkForUpdates,
      api.downloadUpdate,
      api.quitAndInstall,
      api.getUpdateStatus,
      api.getTelemetryStatus,
      api.getTokenUsage,
      api.resetTokenUsage,
      api.getRecentLogs,
      api.getProviderHealth,
      api.getOpenRouterQuota,
    ]
    for (const getter of getters) {
      ipcRendererInvoke.mockClear()
      getter()
      expect(ipcRendererInvoke).toHaveBeenCalledTimes(1)
    }
  })

  it('remaining event subscriptions register exactly one listener each and forward payloads untouched', () => {
    const api = mod.createMikaRuntimeApi()
    const subs = [
      api.onConfigChanged,
      api.onChatStreamChunk,
      api.onChatThinkingStart,
      api.onChatThinkingStop,
      api.onAssistantMessage,
      api.onAvatarExpression,
      api.onToolCall,
      api.onToolResult,
      api.onScreenObservation,
      api.onActivity,
      api.onMoodChanged,
      api.onForceStopTts,
      api.onSensesPaused,
      api.onInterruption,
      api.onNotification,
      api.onPersonCatalogUpdated,
      api.onPersonCatalogDeleted,
      api.onPowerModeChanged,
      api.onPowerProfile,
      api.onBrainProactiveThought,
      api.onChatError,
      api.onToolApprovalRequest,
      api.onProviderHealth,
    ]
    for (const sub of subs) {
      ipcRendererOn.mockClear()
      const callback = vi.fn()
      sub(callback as never)
      expect(ipcRendererOn.mock.calls.length).toBeGreaterThanOrEqual(1)
      const [, listener] = ipcRendererOn.mock.calls[0] as [string, (event: unknown, payload: unknown) => void]
      listener(undefined, { some: 'payload' })
      expect(callback).toHaveBeenCalled()
    }
  })

  it('sendMicState forwards the enabled flag as one payload', () => {
    const api = mod.createMikaRuntimeApi()
    api.sendMicState(true)
    expect(ipcRendererSend).toHaveBeenCalledWith(DESKTOP_SEND_CHANNELS.VoiceMicState, { enabled: true })
  })

  it('getElevenLabsVoices, testPlugin, prepareWizardAsset, getLlmModels and savePrompt/resetPrompt forward their payloads', () => {
    const api = mod.createMikaRuntimeApi()
    api.getElevenLabsVoices({ apiKey: 'k' })
    api.testPlugin({ pluginId: 'p' })
    api.prepareWizardAsset({} as never)
    api.getLlmModels({ provider: 'ollama' })
    api.savePrompt({ namespace: 'n', key: 'k', text: 't' })
    api.resetPrompt({ namespace: 'n', key: 'k' })
    api.getToolsByPlugin('p1')
    expect(ipcRendererInvoke).toHaveBeenCalledTimes(7)
  })
})
