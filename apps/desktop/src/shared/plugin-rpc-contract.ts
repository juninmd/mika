import { z } from 'zod'

export enum PluginRpcErrorCode {
  InvalidRequest = 'invalid-request',
  Unauthorized = 'unauthorized',
  PluginNotReady = 'plugin-not-ready',
  HandlerMissing = 'handler-missing',
  HandlerFailed = 'handler-failed',
}

export const pluginRpcRequestSchema = z.object({
  pluginId: z
    .string()
    .min(1)
    .max(120)
    .regex(/^mika\.[a-z0-9.-]+$/),
  method: z
    .string()
    .min(1)
    .max(120)
    .regex(/^[A-Za-z0-9._:-]+$/),
  args: z.unknown().optional(),
})

const pluginRpcFailureSchema = z.object({
  success: z.literal(false),
  error: z.object({
    code: z.nativeEnum(PluginRpcErrorCode),
    message: z.string().min(1),
  }),
})

const pluginRpcSuccessSchema = z.object({
  success: z.literal(true),
  data: z.unknown().optional(),
})

export const pluginRpcResponseSchema = z.union([pluginRpcSuccessSchema, pluginRpcFailureSchema])

export type PluginRpcRequest = z.infer<typeof pluginRpcRequestSchema>
export type PluginRpcResponse = z.infer<typeof pluginRpcResponseSchema>

export function createPluginRpcRequest(pluginId: string, method: string, args?: unknown): PluginRpcRequest {
  return pluginRpcRequestSchema.parse({ pluginId, method, args })
}

export function createPluginRpcFailure(code: PluginRpcErrorCode, message: string): PluginRpcResponse {
  return {
    success: false,
    error: {
      code,
      message,
    },
  }
}

export function unwrapPluginRpcResponse<T = unknown>(response: PluginRpcResponse): T {
  if (response.success) return response.data as T

  const error = new Error(response.error.message) as Error & { code?: PluginRpcErrorCode }
  error.code = response.error.code
  throw error
}
