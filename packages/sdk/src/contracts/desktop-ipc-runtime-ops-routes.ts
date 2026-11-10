import { z } from 'zod'
import { DESKTOP_INVOKE_CHANNELS } from './desktop-ipc-channels'
import { emptyPayloadSchema } from './desktop-ipc-core-schemas'
import {
  desktopPluginUiCatalogResponseSchema,
  desktopRendererLogRequestSchema,
  desktopWizardPrepareAssetRequestSchema,
  desktopWizardPrepareAssetResponseSchema,
  desktopWizardReactRequestSchema,
  desktopWizardReactResponseSchema,
} from './desktop-ipc-plugin-schemas'

export const DESKTOP_RUNTIME_OPS_ROUTES = {
  pluginUiCatalog: {
    channel: DESKTOP_INVOKE_CHANNELS.PluginUiCatalog,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: desktopPluginUiCatalogResponseSchema,
  },
  elevenLabsVoices: {
    channel: DESKTOP_INVOKE_CHANNELS.ElevenLabsVoices,
    namespace: 'runtime',
    requestSchema: z.object({ apiKey: z.string().min(1) }),
    responseSchema: z.object({
      ok: z.boolean(),
      voices: z.array(z.unknown()).optional(),
      error: z.string().optional(),
    }),
  },
  updateCheck: {
    channel: DESKTOP_INVOKE_CHANNELS.UpdateCheck,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      success: z.boolean(),
      updateAvailable: z.boolean(),
      version: z.string().optional(),
      error: z.string().optional(),
    }),
  },
  updateDownload: {
    channel: DESKTOP_INVOKE_CHANNELS.UpdateDownload,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      success: z.boolean(),
      error: z.string().optional(),
    }),
  },
  updateQuitAndInstall: {
    channel: DESKTOP_INVOKE_CHANNELS.UpdateQuitAndInstall,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      success: z.boolean(),
      error: z.string().optional(),
    }),
  },
  updateStatus: {
    channel: DESKTOP_INVOKE_CHANNELS.UpdateStatus,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      available: z.boolean(),
      version: z.string().optional(),
      progress: z.number().optional(),
    }),
  },
  telemetryOptIn: {
    channel: DESKTOP_INVOKE_CHANNELS.TelemetryOptIn,
    namespace: 'runtime',
    requestSchema: z.object({ enabled: z.boolean(), optInShown: z.boolean() }),
    responseSchema: z.object({ success: z.boolean() }),
  },
  telemetryGetStatus: {
    channel: DESKTOP_INVOKE_CHANNELS.TelemetryGetStatus,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      enabled: z.boolean(),
      optInShown: z.boolean(),
      sessionId: z.string(),
    }),
  },
  tokenUsageGet: {
    channel: DESKTOP_INVOKE_CHANNELS.TokenUsageGet,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({
      totalTokens: z.number().nonnegative(),
      totalPromptTokens: z.number().nonnegative(),
      totalCompletionTokens: z.number().nonnegative(),

      totalCachedPromptTokens: z.number().nonnegative().optional(),
      unknownCostTurns: z.number().int().nonnegative().optional(),
      totalCostUsd: z.number().nonnegative(),
      totalInteractions: z.number().int().nonnegative(),
      firstUsedAt: z.number(),
      lastUsedAt: z.number(),
    }),
  },
  tokenUsageReset: {
    channel: DESKTOP_INVOKE_CHANNELS.TokenUsageReset,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: z.object({ success: z.boolean() }),
  },
  rendererLog: {
    channel: DESKTOP_INVOKE_CHANNELS.RendererLog,
    namespace: 'runtime',
    requestSchema: desktopRendererLogRequestSchema,
    responseSchema: z.object({ ok: z.boolean() }),
  },
  wizardPrepareAsset: {
    channel: DESKTOP_INVOKE_CHANNELS.WizardPrepareAsset,
    namespace: 'runtime',
    requestSchema: desktopWizardPrepareAssetRequestSchema,
    responseSchema: desktopWizardPrepareAssetResponseSchema,
  },
  wizardReact: {
    channel: DESKTOP_INVOKE_CHANNELS.WizardReact,
    namespace: 'runtime',
    requestSchema: desktopWizardReactRequestSchema,
    responseSchema: desktopWizardReactResponseSchema,
  },
} as const
