import type {
  ConfigChangeEvent,
  FeatureSettingsDeclaration,
  LLMModelCatalogItem,
  McpOpenConfigDirResponse,
  McpServerRemoveResponse,
  McpServerSaveRequest,
  McpServerSaveResponse,
  McpServersListResponse,
  McpServerTestRequest,
  McpServerTestResponse,
  WizardReactStep,
} from '@mika/sdk/contracts'

export type PromptSummaryPayload = {
  namespace: string
  key: string
  defaultText: string
  overrideText: string | null
  placeholders: string[]
}

export type SettingsRuntimeApi = {
  getSettingsDeclarations: () => Promise<{ declarations: FeatureSettingsDeclaration[] }>
  getSettingsConfig: () => Promise<{ config: Record<string, unknown> }>
  /** Idioma e tema — as duas preferências que TODA janela lê, inclusive as de plugin. */
  getUiPrefs: () => Promise<{ locale?: string; theme?: string }>
  saveSetting: (
    path: string,
    value: unknown,
  ) => Promise<{
    success: boolean
    change: ConfigChangeEvent
    error?: string
  }>
  getPrompts: () => Promise<{ prompts: PromptSummaryPayload[] }>
  savePrompt: (payload: {
    namespace: string
    key: string
    text: string
  }) => Promise<{ ok: boolean; reset: boolean; droppedPlaceholders: string[]; error?: string }>
  resetPrompt: (payload: { namespace: string; key: string }) => Promise<{ ok: boolean; error?: string }>
  getLlmModels: (payload: {
    provider?: string
    apiKey?: string
    baseUrl?: string
    kind?: 'chat' | 'embedding'
  }) => Promise<{ models: LLMModelCatalogItem[]; error?: string }>
  getElevenLabsVoices: (payload: { apiKey: string }) => Promise<{ ok: boolean; voices?: unknown[]; error?: string }>
  testPlugin: (payload: {
    tabId?: string
    pluginId?: string
    groupPrefix?: string
    prompt?: string
    config?: Record<string, unknown>
  }) => Promise<{
    ok: boolean
    status?: number
    message?: string
    audio?: string
  }>
  wizardReact: (payload: { step: WizardReactStep }) => Promise<{ text: string }>
  prepareWizardAsset: (payload: {
    assetId: 'vad' | 'whisper' | 'nemotron' | 'local-embedding' | 'tts' | 'ollama-llm' | 'ollama-embedding'
    ttsProvider?: string
    ttsVoice?: string
    llmProvider?: string
    llmModel?: string
    embeddingModel?: string
  }) => Promise<{
    success: boolean
    message?: string
    skipped?: boolean
  }>
  onConfigChanged: (callback: (change: ConfigChangeEvent) => void) => () => void
  /** Servidores de `mcp.json` com status vivo e tools; `env` só como chaves (`envKeys`). */
  getMcpServers: () => Promise<McpServersListResponse>
  /** Cria (sem `originalName`) ou edita/renomeia; grava atômico e reinicia só esse servidor. */
  saveMcpServer: (payload: McpServerSaveRequest) => Promise<McpServerSaveResponse>
  removeMcpServer: (payload: { name: string }) => Promise<McpServerRemoveResponse>
  /** Sobe, lista tools e fecha — nada é gravado. */
  testMcpServer: (payload: McpServerTestRequest) => Promise<McpServerTestResponse>
  openMcpConfigDir: () => Promise<McpOpenConfigDirResponse>
  /** Servidor salvo/removido, rodada de conexão terminou ou MCP desligou — releia a lista. */
  onMcpServersChanged: (callback: () => void) => () => void
}
