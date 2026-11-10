import { describe, expect, it } from 'vitest'
import {
  createPluginRpcFailure,
  createPluginRpcRequest,
  PluginRpcErrorCode,
  pluginRpcRequestSchema,
  pluginRpcResponseSchema,
  unwrapPluginRpcResponse,
} from '../plugin-rpc-contract'

describe('plugin RPC contract', () => {
  describe('pluginRpcRequestSchema', () => {
    it('accepts valid plugin RPC requests', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: 'mika.chat',
        method: 'get-history',
        args: { limit: 10 },
      })
      expect(result.success).toBe(true)
    })

    it('accepts requests without args', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: 'mika.chat',
        method: 'ping',
      })
      expect(result.success).toBe(true)
    })

    it('rejects empty pluginId', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: '',
        method: 'ping',
      })
      expect(result.success).toBe(false)
    })

    it('rejects invalid pluginId format', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: 'Invalid_ID',
        method: 'ping',
      })
      expect(result.success).toBe(false)
    })

    it('rejects empty method', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: 'mika.chat',
        method: '',
      })
      expect(result.success).toBe(false)
    })

    it('rejects pluginId exceeding 120 chars', () => {
      const result = pluginRpcRequestSchema.safeParse({
        pluginId: `mika.${'a'.repeat(120)}`,
        method: 'ping',
      })
      expect(result.success).toBe(false)
    })
  })

  describe('pluginRpcResponseSchema', () => {
    it('accepts success response with data', () => {
      const result = pluginRpcResponseSchema.safeParse({
        success: true,
        data: { result: 'ok' },
      })
      expect(result.success).toBe(true)
    })

    it('accepts success response without data', () => {
      const result = pluginRpcResponseSchema.safeParse({
        success: true,
      })
      expect(result.success).toBe(true)
    })

    it('accepts failure response', () => {
      const result = pluginRpcResponseSchema.safeParse({
        success: false,
        error: {
          code: PluginRpcErrorCode.HandlerFailed,
          message: 'Something went wrong',
        },
      })
      expect(result.success).toBe(true)
    })

    it('rejects invalid error code', () => {
      const result = pluginRpcResponseSchema.safeParse({
        success: false,
        error: {
          code: 'unknown-error',
          message: 'fail',
        },
      })
      expect(result.success).toBe(false)
    })
  })

  describe('createPluginRpcRequest', () => {
    it('creates and validates a request', () => {
      const req = createPluginRpcRequest('mika.test', 'do-thing', { foo: 1 })
      expect(req.pluginId).toBe('mika.test')
      expect(req.method).toBe('do-thing')
      expect(req.args).toEqual({ foo: 1 })
    })
  })

  describe('createPluginRpcFailure', () => {
    it('creates a failure response', () => {
      const result = createPluginRpcFailure(PluginRpcErrorCode.HandlerMissing, 'No handler')
      if (result.success) throw new Error('expected a failure response')
      expect(result.error.code).toBe(PluginRpcErrorCode.HandlerMissing)
      expect(result.error.message).toBe('No handler')
    })
  })

  describe('unwrapPluginRpcResponse', () => {
    it('returns data for success response', () => {
      const data = unwrapPluginRpcResponse<{ result: string }>({
        success: true,
        data: { result: 'ok' },
      })
      expect(data).toEqual({ result: 'ok' })
    })

    it('throws for failure response', () => {
      expect(() =>
        unwrapPluginRpcResponse({
          success: false,
          error: { code: PluginRpcErrorCode.HandlerFailed, message: 'fail' },
        }),
      ).toThrow('fail')
    })
  })
})
