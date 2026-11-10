import { describe, expect, it } from 'vitest'
import { redactArgs } from '../redaction'

describe('security and privacy contracts', () => {
  it('redacts provider and integration credentials from telemetry payloads', () => {
    const result = redactArgs(
      {
        apiKey: 'google-secret',
        botToken: 'telegram-secret',
        personalAccessToken: 'github-secret',
        obsPassword: 'obs-secret',
        steamApiKey: 'steam-secret',
        traktClientId: 'trakt-secret',
        openrouterKey: 'openrouter-secret',
        normalValue: 'visible',
      },
      false,
    )

    expect(result).toMatchObject({
      apiKey: '[REDACTED]',
      botToken: '[REDACTED]',
      personalAccessToken: '[REDACTED]',
      obsPassword: '[REDACTED]',
      steamApiKey: '[REDACTED]',
      normalValue: 'visible',
    })
    expect(result.traktClientId).toBe('[REDACTED]')
    expect(result.openrouterKey).toBe('[REDACTED]')
  })

  it('redacts private content by default but preserves it only when explicitly disabled', () => {
    const payload = { query: 'private search', url: 'https://private.example', normalValue: 'visible' }

    expect(redactArgs(payload)).toEqual({
      query: '[REDACTED_CONTENT]',
      url: '[REDACTED_CONTENT]',
      normalValue: 'visible',
    })
    expect(redactArgs(payload, false)).toEqual(payload)
  })

  it('truncates oversized non-secret telemetry values', () => {
    const result = redactArgs({ normalValue: 'x'.repeat(201) }, false)

    expect(result.normalValue).toBe(`${'x'.repeat(200)}...`)
  })
})
