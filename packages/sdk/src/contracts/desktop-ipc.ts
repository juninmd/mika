import { z } from 'zod'
import { DESKTOP_SEND_CHANNELS, DESKTOP_SHELL_CHANNELS } from './desktop-ipc-channels'
import { desktopPluginViewRequestSchema, emptyPayloadSchema, voidResponseSchema } from './desktop-ipc-core-schemas'
import { DESKTOP_MCP_ROUTES } from './desktop-ipc-mcp-schemas'
import { DESKTOP_RUNTIME_OPS_ROUTES } from './desktop-ipc-runtime-ops-routes'
import { DESKTOP_RUNTIME_CORE_ROUTES } from './desktop-ipc-runtime-routes'

export {
  DESKTOP_BRIDGED_EVENT_CHANNELS,
  DESKTOP_EVENT_CHANNELS,
  DESKTOP_INVOKE_CHANNELS,
  DESKTOP_RENDERER_EVENT_CHANNELS,
  DESKTOP_SEND_CHANNELS,
  DESKTOP_SHELL_CHANNELS,
} from './desktop-ipc-channels'
export {
  desktopPluginViewRequestSchema,
  type InferenceAcceleration,
  type PowerProfileState,
} from './desktop-ipc-core-schemas'
export {
  type McpOpenConfigDirResponse,
  type McpServerRemoveResponse,
  type McpServerSaveRequest,
  type McpServerSaveResponse,
  type McpServerSpecInput,
  type McpServerSummary,
  type McpServersListResponse,
  type McpServerTestRequest,
  type McpServerTestResponse,
  mcpServerSaveRequestSchema,
  mcpServerSpecInputSchema,
  mcpServerTestRequestSchema,
} from './desktop-ipc-mcp-schemas'
export {
  desktopRendererLogRequestSchema,
  desktopWizardPrepareAssetRequestSchema,
  desktopWizardPrepareAssetResponseSchema,
  desktopWizardReactRequestSchema,
  desktopWizardReactResponseSchema,
  PROMPT_OVERRIDE_MAX_CHARS,
  type WizardReactStep,
  wizardReactStepSchema,
} from './desktop-ipc-plugin-schemas'

export const DESKTOP_RUNTIME_INVOKE_ROUTES = {
  ...DESKTOP_RUNTIME_CORE_ROUTES,
  ...DESKTOP_RUNTIME_OPS_ROUTES,
  ...DESKTOP_MCP_ROUTES,
} as const

export const DESKTOP_SHELL_INVOKE_ROUTES = {
  openSettings: {
    channel: DESKTOP_SHELL_CHANNELS.OpenSettings,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: voidResponseSchema,
  },
  windowClose: {
    channel: DESKTOP_SHELL_CHANNELS.WindowClose,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: voidResponseSchema,
  },
  windowToggleAlwaysOnTop: {
    channel: DESKTOP_SHELL_CHANNELS.WindowToggleAlwaysOnTop,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.boolean(),
  },
  windowToggleFullscreen: {
    channel: DESKTOP_SHELL_CHANNELS.WindowToggleFullscreen,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.boolean(),
  },
  windowGetFullscreen: {
    channel: DESKTOP_SHELL_CHANNELS.WindowGetFullscreen,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.boolean(),
  },
  autoLaunchGet: {
    channel: DESKTOP_SHELL_CHANNELS.AutoLaunchGet,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.boolean(),
  },
  autoLaunchSet: {
    channel: DESKTOP_SHELL_CHANNELS.AutoLaunchSet,
    namespace: 'shell',
    requestSchema: z.boolean(),
    responseSchema: voidResponseSchema,
  },
  pluginViewOpen: {
    channel: DESKTOP_SHELL_CHANNELS.PluginViewOpen,
    namespace: 'shell',
    requestSchema: desktopPluginViewRequestSchema,
    responseSchema: voidResponseSchema,
  },
  // Host allowlist lives in Rust (`shell_actions::EXTERNAL_HOSTS`); here only the shape.
  openExternal: {
    channel: DESKTOP_SHELL_CHANNELS.OpenExternal,
    namespace: 'shell',
    requestSchema: z.object({ url: z.string().url().max(2048) }),
    responseSchema: voidResponseSchema,
  },
  // A closed folder name, never a path: the Rust side resolves it under the app data dir.
  revealPath: {
    channel: DESKTOP_SHELL_CHANNELS.RevealPath,
    namespace: 'shell',
    requestSchema: z.object({ kind: z.enum(['data', 'logs']) }),
    responseSchema: z.object({ path: z.string() }),
  },
  // Encerra o app inteiro (não só a janela): quem recusa o EULA no primeiro uso sai por aqui.
  appQuit: {
    channel: DESKTOP_SHELL_CHANNELS.AppQuit,
    namespace: 'shell',
    requestSchema: emptyPayloadSchema,
    responseSchema: voidResponseSchema,
  },
} as const

export const DESKTOP_SHELL_EVENT_ROUTES = {
  windowIgnoreMouse: {
    channel: DESKTOP_SHELL_CHANNELS.WindowIgnoreMouse,
    namespace: 'shell',
    payloadSchema: z.boolean(),
  },
  windowMove: {
    channel: DESKTOP_SHELL_CHANNELS.WindowMove,
    namespace: 'shell',
    // Delta por evento de ponteiro: limitado ao maior salto plausível num frame. Sem `finite`,
    // um NaN/Infinity vindo do renderer jogaria a janela para fora de qualquer monitor.
    payloadSchema: z.object({
      deltaX: z.number().finite().min(-4000).max(4000),
      deltaY: z.number().finite().min(-4000).max(4000),
    }),
  },
  // Hands the drag to the OS move loop: smooth, DPI-correct across monitors, no IPC per frame.
  windowStartDrag: {
    channel: DESKTOP_SHELL_CHANNELS.WindowStartDrag,
    namespace: 'shell',
    payloadSchema: z.undefined(),
  },
} as const

/** Documentos legais empacotados, na ordem em que a tela Sobre/Licenças os lista. */
export const LEGAL_DOCUMENT_IDS = ['eula', 'privacy', 'notices', 'assets', 'license'] as const
export type LegalDocumentId = (typeof LEGAL_DOCUMENT_IDS)[number]

export const DESKTOP_LOCAL_INVOKE_ROUTES = {
  systemGetRecentLogs: {
    channel: 'system:getRecentLogs',
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ logs: z.array(z.unknown()) }),
  },
  llmGetProviderHealth: {
    channel: 'llm:getProviderHealth',
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ health: z.array(z.unknown()) }),
  },
  llmResetProviderCircuit: {
    channel: 'llm:resetProviderCircuit',
    namespace: 'runtime',
    requestSchema: z.object({ provider: z.string().min(1), model: z.string().optional() }),
    responseSchema: z.object({ ok: z.boolean() }),
  },
  llmGetOpenRouterQuota: {
    channel: 'llm:getOpenRouterQuota',
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ ok: z.boolean(), quota: z.unknown().optional(), error: z.string().optional() }),
  },
  llmGetModelProfile: {
    channel: 'llm:getModelProfile',
    namespace: 'runtime',
    requestSchema: z.object({ provider: z.string().min(1), model: z.string().min(1).max(256) }),
    responseSchema: z.object({
      contextWindow: z.number().nullable(),
      maxOutputTokens: z.number().nullable(),
      reasoning: z.boolean().nullable(),
      topP: z.boolean().nullable(),
      thinkingBudgetMax: z.number().nullable(),
    }),
  },
  llmGetStatus: {
    channel: 'llm:getStatus',
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ provider: z.string(), ready: z.boolean() }).passthrough(),
  },
  llmPullModel: {
    channel: 'llm:pullModel',
    namespace: 'runtime',
    requestSchema: z.object({ model: z.string().trim().min(1).max(128) }),
    responseSchema: z.object({ ok: z.boolean(), error: z.string().nullable().optional() }),
  },
  llmCancelPull: {
    channel: 'llm:cancelPull',
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ ok: z.boolean() }),
  },
  credentialsTest: {
    channel: 'credentials:test',
    namespace: 'runtime',
    requestSchema: z.object({
      provider: z.enum(['groq', 'openaiStt', 'azure', 'googleTts', 'steam']),
      apiKey: z.string().max(512).optional(),
      region: z.string().max(40).optional(),
    }),
    responseSchema: z.object({
      ok: z.boolean(),
      status: z.number().optional(),
      latencyMs: z.number().optional(),
      count: z.number().nullable().optional(),
      error: z.string().optional(),
    }),
  },
  // Lê o documento legal EMPACOTADO (`bundle.resources` → `legal/`). O id é um enum fechado e o Rust
  // resolve o arquivo (`modules/legal.rs`): o payload nunca carrega caminho.
  legalGetDocument: {
    channel: 'legal:getDocument',
    namespace: 'runtime',
    requestSchema: z.object({ doc: z.enum(LEGAL_DOCUMENT_IDS) }),
    responseSchema: z.object({ doc: z.enum(LEGAL_DOCUMENT_IDS), text: z.string(), version: z.string().min(1) }),
  },
  systemPreventSleep: {
    channel: 'system:preventSleep',
    namespace: 'runtime',
    requestSchema: z.object({ enabled: z.boolean() }),
    responseSchema: z.object({ ok: z.boolean() }),
  },
} as const

// 1s de áudio a 16kHz — espelha `MAX_SAMPLES_PER_CALL`
// (`apps/desktop/src-tauri/src/modules/voice/vad_audio.rs`), bem acima do que o AudioWorklet do
// renderer manda por callback. `voice:vad-audio` roda em até 100 Hz (ver o cabeçalho daquele
// arquivo), então sem este teto um payload malformado de milhões de amostras chegaria ao Rust a
// cada chamada.
const MAX_VAD_SAMPLES_PER_CALL = 16_000

const desktopVadAudioPayloadSchema = z.object({
  samples: z
    .union([z.instanceof(Float32Array), z.array(z.number())])
    .refine((samples) => samples.length <= MAX_VAD_SAMPLES_PER_CALL, {
      message: `samples excede o teto de ${MAX_VAD_SAMPLES_PER_CALL} amostras por chamada`,
    }),
  amplitude: z.number(),
  /** Renderer still playing Mika's audio — keeps barge-in armed after synthesis ends. */
  playing: z.boolean().optional(),
})

export const DESKTOP_LOCAL_EVENT_ROUTES = {
  toolApprovalResponse: {
    channel: DESKTOP_SEND_CHANNELS.ToolApprovalResponse,
    namespace: 'runtime',
    payloadSchema: z.object({
      toolCallId: z.string().min(1),
      approved: z.boolean(),
      scope: z.enum(['once', 'session', 'always']).optional(),
    }),
  },
  voiceMicState: {
    channel: DESKTOP_SEND_CHANNELS.VoiceMicState,
    namespace: 'runtime',
    payloadSchema: z.object({ enabled: z.boolean() }),
  },
  voiceMicStateRequest: {
    channel: DESKTOP_SEND_CHANNELS.VoiceMicStateRequest,
    namespace: 'runtime',
    payloadSchema: emptyPayloadSchema,
  },
  voiceVadAudio: {
    channel: DESKTOP_SEND_CHANNELS.VoiceVadAudio,
    namespace: 'runtime',
    payloadSchema: desktopVadAudioPayloadSchema,
  },
} as const
