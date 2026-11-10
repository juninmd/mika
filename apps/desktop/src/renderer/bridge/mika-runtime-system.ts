import {
  DESKTOP_INVOKE_CHANNELS,
  DESKTOP_LOCAL_INVOKE_ROUTES,
  DESKTOP_RENDERER_EVENT_CHANNELS,
  DESKTOP_SEND_CHANNELS,
  type LegalDocumentId,
} from '@mika/sdk/contracts'
import type { LogItem } from '@mika/sdk/core/logger-format'
import type {
  CredentialTestRequest,
  CredentialTestResult,
  LLMProviderHealth,
  LlmModelProfile,
  LlmPullProgress,
  LlmStatus,
  OpenRouterKeyStatus,
} from '../../shared/electron-api-system-types'
import type { SystemRuntimeApi } from '../../shared/electron-api-types'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { onChannel, sendRendererLog } from './mika-runtime-channel'

export function createSystemRuntimeApi(): SystemRuntimeApi {
  return {
    checkForUpdates: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.UpdateCheck) as Promise<{
        success: boolean
        updateAvailable: boolean
        version?: string
        notes?: string
        error?: string
      }>,
    downloadUpdate: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.UpdateDownload) as Promise<{
        success: boolean
        error?: string
      }>,
    quitAndInstall: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.UpdateQuitAndInstall) as Promise<{
        success: boolean
        error?: string
      }>,
    getUpdateStatus: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.UpdateStatus) as Promise<{
        available: boolean
        version?: string
        progress?: number
      }>,
    getLegalDocument: (doc) =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.legalGetDocument.channel, { doc }) as Promise<{
        doc: LegalDocumentId
        text: string
        version: string
      }>,
    getTelemetryStatus: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.TelemetryGetStatus) as Promise<{
        enabled: boolean
        optInShown: boolean
        sessionId: string
      }>,
    setTelemetryOptIn: (payload: { enabled: boolean; optInShown: boolean }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.TelemetryOptIn, payload) as Promise<{ success: boolean }>,
    getTokenUsage: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.TokenUsageGet) as Promise<{
        totalTokens: number
        totalPromptTokens: number
        totalCompletionTokens: number
        totalCostUsd: number
        totalInteractions: number
        firstUsedAt: number
        lastUsedAt: number
      }>,
    resetTokenUsage: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.TokenUsageReset) as Promise<{ success: boolean }>,
    onToolApprovalRequest: (
      callback: (payload: { toolCallId: string; toolName: string; args: Record<string, unknown> }) => void,
    ) => onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.ToolApprovalRequest, callback),
    sendToolApprovalResponse: (toolCallId: string, approved: boolean, scope?: 'once' | 'session' | 'always') => {
      ipcRenderer.send(DESKTOP_SEND_CHANNELS.ToolApprovalResponse, { toolCallId, approved, scope })
    },
    log: sendRendererLog,
    getRecentLogs: () =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.systemGetRecentLogs.channel) as Promise<{ logs: LogItem[] }>,

    onLog: (callback: (log: LogItem) => void) =>
      onChannel<LogItem[]>(DESKTOP_RENDERER_EVENT_CHANNELS.SystemLogBatch, (batch) => {
        for (const item of batch) callback(item)
      }),
    getProviderHealth: () =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmGetProviderHealth.channel) as Promise<{
        health: LLMProviderHealth[]
      }>,
    resetProviderCircuit: (provider: string, model?: string) =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmResetProviderCircuit.channel, { provider, model }) as Promise<{
        ok: boolean
      }>,
    onProviderHealth: (callback: (health: LLMProviderHealth) => void) =>
      onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.LlmProviderHealth, callback),
    getOpenRouterQuota: () =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmGetOpenRouterQuota.channel) as Promise<{
        ok: boolean
        quota?: OpenRouterKeyStatus
        error?: string
      }>,
    testCredential: (payload: CredentialTestRequest) =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.credentialsTest.channel, payload) as Promise<CredentialTestResult>,
    getModelProfile: (provider: string, model: string) =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmGetModelProfile.channel, {
        provider,
        model,
      }) as Promise<LlmModelProfile>,
    getLlmStatus: () => ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmGetStatus.channel) as Promise<LlmStatus>,
    onLlmStatus: (callback: (status: LlmStatus) => void) =>
      onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.LlmStatus, callback),
    pullLlmModel: (model: string) =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmPullModel.channel, { model }) as Promise<{
        ok: boolean
        error?: string | null
      }>,
    cancelLlmPull: () =>
      ipcRenderer.invoke(DESKTOP_LOCAL_INVOKE_ROUTES.llmCancelPull.channel) as Promise<{ ok: boolean }>,
    onLlmPullProgress: (callback: (progress: LlmPullProgress) => void) =>
      onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.LlmPullProgress, callback),
  }
}
