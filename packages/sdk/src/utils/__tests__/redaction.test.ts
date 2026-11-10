import { describe, expect, it } from 'vitest'
import { redactArgs, summarizeResult } from '../redaction'

describe('redaction utilities', () => {
  describe('redactArgs', () => {
    it('returns empty object for undefined or null inputs', () => {
      expect(redactArgs(undefined)).toEqual({})

      expect(redactArgs(null)).toEqual({})
    })

    it('returns empty object for empty object input', () => {
      expect(redactArgs({})).toEqual({})
    })

    it('redacts sensitive keys', () => {
      const args = {
        token: 'my-token',
        password: 'my-password',
        authorization: 'Bearer token',
        cookie: 'session=123',
        api_key: 'my-api-key',
        CLIENT_ID: 'my-client-id',
        session: 'my-session',
        privateKey: 'my-private-key',
        openrouter_key: 'my-openrouter-key',
      }
      const expected = {
        token: '[REDACTED]',
        password: '[REDACTED]',
        authorization: '[REDACTED]',
        cookie: '[REDACTED]',
        api_key: '[REDACTED]',
        CLIENT_ID: '[REDACTED]',
        session: '[REDACTED]',
        privateKey: '[REDACTED]',
        openrouter_key: '[REDACTED]',
      }
      expect(redactArgs(args)).toEqual(expected)
    })

    it('redacts private content keys by default', () => {
      const args = {
        query: 'search query',
        url: 'https://example.com',
        history: 'browsing history',
        recentwatched: 'video',
        recentsearches: 'searches',
        title: 'page title',
      }
      const expected = {
        query: '[REDACTED_CONTENT]',
        url: '[REDACTED_CONTENT]',
        history: '[REDACTED_CONTENT]',
        recentwatched: '[REDACTED_CONTENT]',
        recentsearches: '[REDACTED_CONTENT]',
        title: '[REDACTED_CONTENT]',
      }
      expect(redactArgs(args)).toEqual(expected)
    })

    it('preserves private content keys if includePrivateContent is false', () => {
      const args = {
        query: 'search query',
        url: 'https://example.com',
      }
      expect(redactArgs(args, false)).toEqual(args)
    })

    it('truncates strings longer than MAX_VALUE_LENGTH (200 characters)', () => {
      const longString = 'a'.repeat(250)
      const args = {
        longText: longString,
      }
      const result = redactArgs(args)
      expect(result.longText).toBe(`${'a'.repeat(200)}...`)
      expect((result.longText as string).length).toBe(203)
    })

    it('preserves normal keys and non-string values', () => {
      const args = {
        name: 'John Doe',
        age: 30,
        isActive: true,
        nested: { foo: 'bar' },
        items: [1, 2, 3],
      }
      expect(redactArgs(args)).toEqual(args)
    })

    it('handles a mix of keys', () => {
      const args = {
        username: 'user1',
        password: 'securePassword123',
        query: 'test query',
        data: 'a'.repeat(250),
      }
      const expected = {
        username: 'user1',
        password: '[REDACTED]',
        query: '[REDACTED_CONTENT]',
        data: `${'a'.repeat(200)}...`,
      }
      expect(redactArgs(args)).toEqual(expected)
    })

    it('recursively redacts credentials nested inside objects', () => {
      const args = {
        headers: { Authorization: 'Bearer sk-secret' },
        body: { nested: { apiKey: 'sk-nested' } },
        ok: true,
      }
      expect(redactArgs(args)).toEqual({
        headers: { Authorization: '[REDACTED]' },
        body: { nested: { apiKey: '[REDACTED]' } },
        ok: true,
      })
    })

    it('recursively redacts credentials inside arrays', () => {
      const args = {
        messages: [{ role: 'user', content: 'hi' }, { token: 'abc123' }],
      }
      expect(redactArgs(args)).toEqual({
        messages: [{ role: 'user', content: 'hi' }, { token: '[REDACTED]' }],
      })
    })

    it('applies private-content and truncation rules recursively', () => {
      const args = {
        meta: { query: 'search', url: 'https://example.com' },
        log: 'x'.repeat(250),
      }
      const result = redactArgs(args)
      expect(result.meta).toEqual({ query: '[REDACTED_CONTENT]', url: '[REDACTED_CONTENT]' })
      expect(result.log).toBe(`${'x'.repeat(200)}...`)
    })

    it('keeps private content keys nested when includePrivateContent is false', () => {
      const args = { meta: { query: 'search' } }
      expect(redactArgs(args, false)).toEqual(args)
    })
  })

  describe('summarizeResult', () => {
    it('returns resultType for non-object or null inputs', () => {
      expect(summarizeResult(null)).toEqual({ resultType: 'object' })
      expect(summarizeResult(undefined)).toEqual({ resultType: 'undefined' })
      expect(summarizeResult('string')).toEqual({ resultType: 'string' })
      expect(summarizeResult(123)).toEqual({ resultType: 'number' })
      expect(summarizeResult(true)).toEqual({ resultType: 'boolean' })
    })

    it('summarizes object with data and success', () => {
      const input = {
        success: true,
        data: { id: 1 },
        otherInfo: 'ignored',
      }
      expect(summarizeResult(input)).toEqual({
        success: true,
        hasData: true,
        hasError: false,
      })
    })

    it('summarizes object with error', () => {
      const input = {
        success: false,
        error: 'Something went wrong',
      }
      expect(summarizeResult(input)).toEqual({
        success: false,
        hasData: false,
        hasError: true,
      })
    })

    it('handles objects missing success, data, and error', () => {
      const input = {
        foo: 'bar',
      }
      expect(summarizeResult(input)).toEqual({
        success: undefined,
        hasData: false,
        hasError: false,
      })
    })

    it('handles non-boolean success values', () => {
      const input = {
        success: 'yes',
        data: [],
      }
      expect(summarizeResult(input)).toEqual({
        success: undefined,
        hasData: true,
        hasError: false,
      })
    })
  })
})
