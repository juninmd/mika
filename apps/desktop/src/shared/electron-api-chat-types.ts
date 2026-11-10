import type {
  ChatAttachment,
  ChatMessageSource,
  CrisisRegion,
  PluginUiCatalog,
  ToolResult,
  UsageMetadata,
  VTuberExpression,
} from '@mika/sdk/contracts'
import type { ToolCallOrigin } from '@mika/sdk/svc/events'
import type { DesktopHealthStatus } from './desktop-health-status'

export type ToolCatalogItem = {
  name: string
  description?: string
  inputSchema?: {
    type: string
    properties: Record<string, { type: string; description: string; enum?: string[] }>
    required?: string[]
  }
  pluginId: string
  requiresWindow: boolean
}

export type ChatRuntimeApi = {
  getHealth: () => Promise<{
    status: DesktopHealthStatus
    uptime: number
    features: number
    settingsTabs: number
    tools: number

    version: string

    packaged: boolean
    updateChannel: 'steam' | 'direct'
    bootstrap: {
      coreMinimumReady: boolean
      pluginsReady: boolean
      degraded: boolean
      errors: string[]
    }
    capabilities: {
      hasLLM: boolean
      canSpeak: boolean
      canListenAudio: boolean
      canSeeScreen: boolean
      canUseTerminal: boolean
      canLearn: boolean
      plugins: number
    }
  }>
  sendChatMessage: (
    message: string,
    attachments?: ChatAttachment[],
    source?: ChatMessageSource,
  ) => Promise<{
    id: string
    content: string
    toolCount: number
    /** `assistant` = she replied; `system` = the turn failed and `content` is the error message. */
    role?: string
    crisisRegion?: CrisisRegion
  }>
  getChatHistory: () => Promise<{
    messages: Array<{ id: string; role: string; content: string; timestamp: number; source?: ChatMessageSource }>
  }>
  clearChatHistory: () => Promise<{ success: boolean }>
  cancelChatMessage: () => Promise<{ success: boolean }>
  /** "Report this response": saves `reports/<ts>.json` on this computer (response text, model, persona) and returns its path. Nothing is sent. */
  reportChatResponse: (content: string) => Promise<{ file: string }>
  transcribeAudio: (payload: {
    audioBase64: string
    mimeType?: string
    language?: string
  }) => Promise<{ success: boolean; data?: { text: string }; error?: string }>
  /**
   * Silence-by-voice fast path (specs/08-gamer-companion.md, "Silêncio é feature"): checked BEFORE
   * a final transcript is sent as a chat turn. A matched command mutes/unmutes instantly on the
   * Rust side (same `MuteSpeech` bus event the shortcut and the `voice` tool publish) and the
   * caller must NOT forward that utterance to `sendChatMessage`.
   */
  checkSilenceCommand: (payload: { text: string }) => Promise<{ matched: boolean; muted?: boolean }>

  sendVadAudio: (payload: { samples: Float32Array | number[]; amplitude: number; playing?: boolean }) => void
  getTools: () => Promise<{ tools: ToolCatalogItem[] }>
  getToolsByPlugin: (pluginId: string) => Promise<{ tools: ToolCatalogItem[] }>
  getPluginUi: () => Promise<PluginUiCatalog>
  executeTool: <T = unknown>(
    name: string,
    args: Record<string, unknown>,
    origin?: ToolCallOrigin,
  ) => Promise<ToolResult<T>>
  onTTSAudio: (
    callback: (payload: {
      audio: string
      mimeType: string
      words?: Array<{ word: string; offset_ms: number; duration_ms: number }>
    }) => void,
  ) => () => void
  onChatStreamChunk: (callback: (payload: { text: string; origin?: ToolCallOrigin }) => void) => () => void
  onChatThinkingStart: (callback: (payload: { userContent: string; origin?: ToolCallOrigin }) => void) => () => void
  onChatThinkingStop: (callback: (payload: Record<string, never>) => void) => () => void
  onAssistantMessage: (
    callback: (payload: { content: string; timestamp: number; usage?: UsageMetadata }) => void,
  ) => () => void
  onAvatarExpression: (callback: (payload: { expression: VTuberExpression; intensity?: number }) => void) => () => void
  onToolCall: (
    callback: (payload: { name: string; args: unknown; callId?: string; at?: number; origin?: ToolCallOrigin }) => void,
  ) => () => void
  onToolResult: (
    callback: (payload: {
      name: string
      result: unknown
      callId?: string
      durationMs?: number
      success?: boolean
      origin?: ToolCallOrigin
    }) => void,
  ) => () => void
  onScreenObservation: (
    callback: (payload: {
      analysis: string
      foregroundApp?: { title?: string; processName?: string }
      isLikelyGame?: boolean
      isFunny?: boolean
      memeContext?: string
      at?: number
    }) => void,
  ) => () => void
}
