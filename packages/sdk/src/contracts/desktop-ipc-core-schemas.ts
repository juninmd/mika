import { z } from 'zod'
import { CRISIS_REGIONS } from './types/runtime-types'

export const emptyPayloadSchema = z.undefined()
export const voidResponseSchema = z.undefined()

export const desktopPluginViewRequestSchema = z.object({
  pluginId: z.string().min(1),
  viewId: z.string().min(1),
})

export const desktopBootstrapStatusSchema = z.object({
  coreMinimumReady: z.boolean(),
  pluginsReady: z.boolean(),
  degraded: z.boolean(),
  errors: z.array(z.string()),
})

export const desktopCapabilitiesSchema = z.object({
  hasLLM: z.boolean(),
  canSpeak: z.boolean(),
  canListenAudio: z.boolean(),
  canSeeScreen: z.boolean(),
  canUseTerminal: z.boolean(),
  canLearn: z.boolean(),
  plugins: z.number().int().nonnegative(),
})

export const desktopHealthSchema = z.object({
  status: z.enum(['degraded', 'ok', 'starting', 'initializing']),
  uptime: z.number().int().nonnegative(),
  features: z.number().int().nonnegative(),
  settingsTabs: z.number().int().nonnegative(),
  tools: z.number().int().nonnegative(),

  version: z.string().min(1),
  packaged: z.boolean(),
  /** Quem atualiza o binário: cliente Steam (build `--features steam`) ou updater do Tauri. */
  updateChannel: z.enum(['steam', 'direct']),
  bootstrap: desktopBootstrapStatusSchema,
  capabilities: desktopCapabilitiesSchema,
})

/** Ids e motivos espelham `Model::id`/`Reason::as_str` (Rust `modules/inference/accel.rs`, que lê este arquivo). */
export const inferenceModelIdSchema = z.enum([
  'whisperEncoder',
  'whisperDecoder',
  'nemotron',
  'supertonic',
  'silero',
  'e5',
])
export const inferenceReasonSchema = z.enum([
  'userCpu',
  'tinyModel',
  'perToken',
  'noGpu',
  'userGpu',
  'measuredCpu',
  'economy',
  'gaming',
  'battery',
  'accelerated',
  'gpuFailed',
])

/** Aceleração dos modelos ONNX: preferência, EP de GPU deste build e o que cada carga de fato usou. */
export const inferenceAccelerationSchema = z.object({
  preference: z.enum(['auto', 'gpu', 'cpu']),
  gpuProvider: z.string().nullable(),
  models: z.array(
    z.object({
      id: inferenceModelIdSchema,
      requested: z.enum(['gpu', 'cpu']),
      provider: z.string(),
      reason: inferenceReasonSchema,
      error: z.string().nullable(),
    }),
  ),
})

export type InferenceAcceleration = z.infer<typeof inferenceAccelerationSchema>

export const powerProfileSchema = z.object({
  profile: z.enum(['auto', 'economy', 'quality']),
  state: z.enum(['active', 'idle', 'gaming', 'battery']),
  ceiling: z.enum(['full', 'reduced', 'eco']),
  backgroundScale: z.number().int().positive(),
  visionScale: z.number().int().positive(),
  onnxThreads: z.number().int().positive(),
  acceleration: inferenceAccelerationSchema,
})

/** Governador de energia (Rust `modules/system/governor`): perfil escolhido, estado detectado e orçamento. */
export type PowerProfileState = z.infer<typeof powerProfileSchema>

export const desktopPowerStateSchema = powerProfileSchema.partial().extend({
  onBatteryPower: z.boolean(),
})

export const chatAttachmentSchema = z.object({
  type: z.literal('image'),
  mimeType: z.string(),
  data: z.string(),
})

// The schema decides what crosses IPC: an undeclared field is silently dropped even with a correct handler/type.
export const chatMessageSourceSchema = z.enum(['voice', 'text'])

export const desktopChatSendRequestSchema = z.object({
  message: z.string(),
  attachments: z.array(chatAttachmentSchema).max(4).optional(),
  source: chatMessageSourceSchema.optional(),
})

export const desktopChatSendResponseSchema = z.object({
  id: z.string().min(1),
  content: z.string(),
  toolCount: z.number().int().nonnegative(),
  /** `assistant` = she replied; `system` = the turn failed and `content` is the error message. */
  role: z.string().min(1),
  /** Set when the user's message carried a self-harm signal (`brain/crisis.rs`): the UI shows the crisis-resources card for this region. */
  crisisRegion: z.enum(CRISIS_REGIONS).optional(),
})

export const desktopChatHistoryResponseSchema = z.object({
  messages: z.array(
    z.object({
      id: z.string(),
      role: z.string(),
      content: z.string(),
      timestamp: z.number(),
      source: chatMessageSourceSchema.optional(),
    }),
  ),
})

// "Report this response": writes `reports/<ts>.json` locally (response text + model + persona; never screen, audio or the user's message).
export const desktopChatReportRequestSchema = z.object({
  content: z.string().min(1).max(20000),
})

export const desktopChatReportResponseSchema = z.object({
  /** Absolute path of the report file, shown to the user so they can hand it to whoever they choose. */
  file: z.string().min(1),
})

export const desktopChatCancelResponseSchema = z.object({
  success: z.boolean(),
})

export const desktopChatClearResponseSchema = z.object({
  success: z.boolean(),
})

// ~5min de PCM16 mono a 16kHz em base64 — espelha `MAX_AUDIO_BASE64_CHARS`
// (`apps/desktop/src-tauri/src/modules/voice/transcribe.rs`). Os dois lados têm de rejeitar o
// mesmo payload oversized: sem o `.max()` aqui, um cliente TS confiaria no schema e só descobriria
// o teto na resposta de erro do Rust.
const MAX_AUDIO_BASE64_CHARS = 16_000_000

export const desktopTranscribeRequestSchema = z.object({
  audioBase64: z.string().min(1).max(MAX_AUDIO_BASE64_CHARS),
  mimeType: z.string().optional(),
  language: z.string().optional(),
})

export const desktopTranscribeResponseSchema = z.object({
  success: z.boolean(),
  data: z.object({ text: z.string() }).optional(),
  error: z.string().optional(),
})

export const desktopSettingsDeclarationsResponseSchema = z.object({
  declarations: z.array(z.unknown()),
})

export const desktopSettingsConfigResponseSchema = z.object({
  config: z.record(z.string(), z.unknown()),
})

/**
 * Appearance preferences every window needs — locale and theme. Nothing else goes here.
 * A brand-new profile already gets the OS language written by the Rust shell on first boot
 * (`config/locale.rs::seed_first_run`), so `locale` is always the language to show.
 */
export const desktopSettingsUiPrefsResponseSchema = z.object({
  locale: z.string().optional(),
  theme: z.string().optional(),
})

export const desktopSettingsSetRequestSchema = z.object({
  path: z.string(),
  value: z.unknown(),
})

export const desktopConfigChangeSchema = z.object({
  path: z.string(),
  value: z.unknown(),
  previousValue: z.unknown().optional(),
  source: z.string(),
  timestamp: z.number(),

  accepted: z.boolean().optional(),
  rejectedReason: z.string().optional(),
})

export const desktopSettingsSetResponseSchema = z.object({
  success: z.boolean(),
  change: desktopConfigChangeSchema,
  error: z.string().optional(),
})
