import { z } from 'zod'

export const desktopPromptSummarySchema = z.object({
  namespace: z.string(),
  key: z.string(),
  defaultText: z.string(),
  overrideText: z.string().nullable(),
  placeholders: z.array(z.string()),
})

export const desktopPromptsListResponseSchema = z.object({
  prompts: z.array(desktopPromptSummarySchema),
})

// Largest real prompt is ~11KB; without a cap, a huge paste becomes the whole context on the next turn.
export const PROMPT_OVERRIDE_MAX_CHARS = 64_000

export const desktopPromptsSaveRequestSchema = z.object({
  namespace: z.string().max(128),
  key: z.string().max(256),
  text: z.string().max(PROMPT_OVERRIDE_MAX_CHARS),
})

export const desktopPromptsSaveResponseSchema = z.object({
  ok: z.boolean(),
  reset: z.boolean(),
  droppedPlaceholders: z.array(z.string()),
  error: z.string().optional(),
})

export const desktopPromptsResetRequestSchema = z.object({
  namespace: z.string(),
  key: z.string(),
})

export const desktopPromptsResetResponseSchema = z.object({
  ok: z.boolean(),
  error: z.string().optional(),
})

export const desktopLlmModelsRequestSchema = z.object({
  provider: z.string().optional(),
  apiKey: z.string().optional(),
  baseUrl: z.string().optional(),
  kind: z.enum(['chat', 'embedding']).optional(),
})

export const desktopLlmModelsResponseSchema = z.object({
  models: z.array(z.unknown()),
  error: z.string().optional(),
})

export const desktopPluginTestRequestSchema = z.object({
  tabId: z.string().optional(),
  pluginId: z.string().optional(),
  groupPrefix: z.string().optional(),
  prompt: z.string().optional(),
  config: z.record(z.string(), z.unknown()).optional(),
})

export const desktopPluginTestResponseSchema = z.object({
  ok: z.boolean(),
  status: z.number().optional(),
  message: z.string().optional(),
  audio: z.string().optional(),
})

export const desktopToolExecuteRequestSchema = z.object({
  name: z.string(),
  args: z.record(z.string(), z.unknown()),
  // Zod drops undeclared fields; without this, `origin` vanishes and every caller becomes 'turn'.
  origin: z.enum(['turn', 'background']).optional(),
})

export const desktopToolsListByPluginRequestSchema = z.object({
  pluginId: z.string(),
})

export const desktopToolCatalogResponseSchema = z.object({
  tools: z.array(z.unknown()),
})

export const desktopPluginUiCatalogResponseSchema = z.object({
  views: z.array(z.unknown()),
  settingsPanels: z.array(z.unknown()),
})

export const desktopRendererLogRequestSchema = z.object({
  level: z.enum(['debug', 'info', 'warn', 'error']),
  scope: z.string().min(1).max(64),
  msg: z.string().min(1).max(300),
  meta: z.record(z.string(), z.unknown()).optional(),
})

export const desktopWizardPrepareAssetRequestSchema = z.object({
  assetId: z.enum(['vad', 'whisper', 'nemotron', 'local-embedding', 'tts', 'ollama-llm', 'ollama-embedding']),
  ttsProvider: z.string().optional(),
  ttsVoice: z.string().optional(),
  llmProvider: z.string().optional(),
  llmModel: z.string().optional(),
  embeddingModel: z.string().optional(),
})

// Only the step crosses the boundary: the backend reads the choices from the saved config, so no
// free text from the wizard ever reaches the prompt.
export const wizardReactStepSchema = z.enum(['avatar', 'llm', 'tts', 'embedding', 'vision', 'identity'])

export type WizardReactStep = z.infer<typeof wizardReactStepSchema>

export const desktopWizardReactRequestSchema = z.object({ step: wizardReactStepSchema })

export const desktopWizardReactResponseSchema = z.object({ text: z.string() })

export const desktopWizardPrepareAssetResponseSchema = z.object({
  success: z.boolean(),
  message: z.string().optional(),
  skipped: z.boolean().optional(),
})
