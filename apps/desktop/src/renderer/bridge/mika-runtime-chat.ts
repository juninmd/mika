import type {
  ChatAttachment,
  ChatMessageSource,
  PluginUiCatalog,
  UsageMetadata,
  VTuberExpression,
} from '@mika/sdk/contracts'
import { DESKTOP_EVENT_CHANNELS, DESKTOP_INVOKE_CHANNELS, DESKTOP_SEND_CHANNELS } from '@mika/sdk/contracts'
import type { ToolCallOrigin } from '@mika/sdk/svc/events'
import type { ChatRuntimeApi, ToolCatalogItem } from '../../shared/electron-api-types'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { onChannel } from './mika-runtime-channel'

export function createChatRuntimeApi(): ChatRuntimeApi {
  return {
    getHealth: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.GetHealth),
    sendChatMessage: (message: string, attachments?: ChatAttachment[], source?: ChatMessageSource) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ChatSend, { message, attachments, source }),
    getChatHistory: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ChatHistory),
    clearChatHistory: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ChatClear),
    cancelChatMessage: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ChatCancel),
    reportChatResponse: (content: string) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ChatReport, { content }) as Promise<{ file: string }>,
    transcribeAudio: (payload: { audioBase64: string; mimeType?: string; language?: string }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.VoiceTranscribe, payload),
    checkSilenceCommand: (payload: { text: string }) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.VoiceSilenceCommand, payload) as Promise<{
        matched: boolean
        muted?: boolean
      }>,

    sendVadAudio: (payload: { samples: Float32Array | number[]; amplitude: number; playing?: boolean }) => {
      ipcRenderer.send(DESKTOP_SEND_CHANNELS.VoiceVadAudio, payload)
    },
    executeTool: (name: string, args: Record<string, unknown>, origin?: ToolCallOrigin) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ToolExecute, { name, args, origin }),
    getTools: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ToolsList) as Promise<{ tools: ToolCatalogItem[] }>,
    getToolsByPlugin: (pluginId: string) =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.ToolsListByPlugin, { pluginId }) as Promise<{
        tools: ToolCatalogItem[]
      }>,
    getPluginUi: () => ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.PluginUiCatalog) as Promise<PluginUiCatalog>,
    onTTSAudio: (
      callback: (payload: {
        audio: string
        mimeType: string
        words?: Array<{ word: string; offset_ms: number; duration_ms: number }>
      }) => void,
    ) => onChannel(DESKTOP_EVENT_CHANNELS.TtsAudio, callback),
    onChatStreamChunk: (callback: (payload: { text: string; origin?: ToolCallOrigin }) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.ChatStreamChunk, callback),
    onChatThinkingStart: (callback: (payload: { userContent: string; origin?: ToolCallOrigin }) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.ChatThinkingStart, callback),
    onChatThinkingStop: (callback: (payload: Record<string, never>) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.ChatThinkingStop, callback),
    onAssistantMessage: (callback: (payload: { content: string; timestamp: number; usage?: UsageMetadata }) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.ChatAssistantMessage, callback),
    onAvatarExpression: (callback: (payload: { expression: VTuberExpression }) => void) =>
      onChannel(DESKTOP_EVENT_CHANNELS.AvatarExpression, callback),
    onToolCall: (
      callback: (payload: {
        name: string
        args: unknown
        callId?: string
        at?: number
        origin?: ToolCallOrigin
      }) => void,
    ) => onChannel(DESKTOP_EVENT_CHANNELS.ChatToolCall, callback),
    onToolResult: (
      callback: (payload: {
        name: string
        result: unknown
        callId?: string
        durationMs?: number
        success?: boolean
        origin?: ToolCallOrigin
      }) => void,
    ) => onChannel(DESKTOP_EVENT_CHANNELS.ChatToolResult, callback),
    onScreenObservation: (
      callback: (payload: {
        analysis: string
        foregroundApp?: { title?: string; processName?: string }
        isLikelyGame?: boolean
        isFunny?: boolean
        memeContext?: string
        at?: number
      }) => void,
    ) => onChannel(DESKTOP_EVENT_CHANNELS.VisionScreenObservation, callback),
  }
}
