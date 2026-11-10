import { describe, expect, it } from 'vitest'
import { redactArgs, redactLogMeta } from '../redaction'

describe('redação — contadores não são segredo', () => {
  it('preserva contagens de token e o booleano de presença de chave', () => {
    expect(
      redactLogMeta({
        totalTokens: 1820,
        promptTokens: 1500,
        firstTokenMs: 780,
        apiKeyPresent: true,
      }),
    ).toEqual({ totalTokens: 1820, promptTokens: 1500, firstTokenMs: 780, apiKeyPresent: true })
  })

  it('continua apagando o segredo de verdade, que é sempre texto', () => {
    expect(redactLogMeta({ apiKey: 'sk-live-abc123', sessionToken: 'eyJhbGciOi' })).toEqual({
      apiKey: '[REDACTED]',
      sessionToken: '[REDACTED]',
    })
  })

  it('vale também para args de tool, aninhados', () => {
    expect(redactArgs({ usage: { totalTokens: 42 }, auth: { token: 'abc' } })).toEqual({
      usage: { totalTokens: 42 },
      auth: { token: '[REDACTED]' },
    })
  })
})
