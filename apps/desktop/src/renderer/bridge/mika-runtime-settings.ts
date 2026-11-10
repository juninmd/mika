import type {
  ConfigChangeEvent,
  FeatureSettingsDeclaration,
  LLMModelCatalogItem,
  McpOpenConfigDirResponse,
  McpServerRemoveResponse,
  McpServerSaveResponse,
  McpServersListResponse,
  McpServerTestResponse,
} from '@mika/sdk/contracts'
import { DESKTOP_EVENT_CHANNELS, DESKTOP_INVOKE_CHANNELS } from '@mika/sdk/contracts'
import type { PromptSummaryPayload, SettingsRuntimeApi } from '../../shared/electron-api-types'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { onChannel } from './mika-runtime-channel'

export function createSettingsRuntimeApi(): SettingsRuntimeApi {
  return {
    getSettingsDeclarations: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.SettingsDeclarations) as Promise<{
        declarations: FeatureSettingsDeclaration[]
      }>,
    getSettingsConfig: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.SettingsGetAll) as Promise<{ config: Record<string, unknown> }>,
    getUiPrefs: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.SettingsUiPrefs) as Promise<{
        locale?: string
        theme?: string
      }>,
    saveSetting: (path: string, value: unknown) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.SettingsSet, { path, value }) as Promise<{
        success: boolean
        change: ConfigChangeEvent
      }>,
    getPrompts: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.PromptsList) as Promise<{ prompts: PromptSummaryPayload[] }>,
    savePrompt: (payload: { namespace: string; key: string; text: string }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.PromptsSave, payload) as Promise<{
        ok: boolean
        reset: boolean
        droppedPlaceholders: string[]
        error?: string
      }>,
    resetPrompt: (payload: { namespace: string; key: string }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.PromptsReset, payload) as Promise<{ ok: boolean; error?: string }>,
    getLlmModels: (payload: { provider?: string; apiKey?: string; baseUrl?: string; kind?: 'chat' | 'embedding' }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.LlmModels, payload) as Promise<{ models: LLMModelCatalogItem[] }>,
    getElevenLabsVoices: (payload: { apiKey: string }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ElevenLabsVoices, payload) as Promise<{
        ok: boolean
        voices?: unknown[]
        error?: string
      }>,
    testPlugin: (payload: {
      tabId?: string
      pluginId?: string
      groupPrefix?: string
      prompt?: string
      config?: Record<string, unknown>
    }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.PluginTest, payload) as Promise<{
        ok: boolean
        status?: number
        message?: string
      }>,
    prepareWizardAsset: (payload) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.WizardPrepareAsset, payload) as Promise<{
        success: boolean
        message?: string
        skipped?: boolean
      }>,
    wizardReact: (payload) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.WizardReact, payload) as Promise<{ text: string }>,
    onConfigChanged: (callback: (change: ConfigChangeEvent) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.ConfigChanged, callback),
    getMcpServers: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.McpServersList) as Promise<McpServersListResponse>,
    saveMcpServer: (payload) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.McpServerSave, payload) as Promise<McpServerSaveResponse>,
    removeMcpServer: (payload) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.McpServerRemove, payload) as Promise<McpServerRemoveResponse>,
    testMcpServer: (payload) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.McpServerTest, payload) as Promise<McpServerTestResponse>,
    openMcpConfigDir: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.McpOpenConfigDir) as Promise<McpOpenConfigDirResponse>,
    onMcpServersChanged: (callback: () => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.McpServersChanged, () => callback()),
  }
}
