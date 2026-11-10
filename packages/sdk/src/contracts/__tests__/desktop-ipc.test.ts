import { describe, expect, it } from 'vitest'
import { DESKTOP_RUNTIME_INVOKE_ROUTES } from '../desktop-ipc'

describe('desktopSettingsSetResponseSchema preserva accepted/rejectedReason', () => {
  const responseSchema = DESKTOP_RUNTIME_INVOKE_ROUTES.settingsSet.responseSchema

  it('mantém accepted:false e rejectedReason numa recusa de escrita', () => {
    const raw = {
      success: false,
      change: {
        path: 'llm.apiKey',
        value: 'x',
        source: 'renderer',
        timestamp: 1,
        accepted: false,
        rejectedReason: 'schema inválido para a raiz "llm"',
      },
      error: 'schema inválido para a raiz "llm"',
    }

    const parsed = responseSchema.parse(raw)

    expect(parsed.change).toMatchObject({ accepted: false, rejectedReason: 'schema inválido para a raiz "llm"' })
  })

  it('mantém accepted:true numa escrita aceita', () => {
    const raw = {
      success: true,
      change: {
        path: 'voice.ttsProvider',
        value: 'supertonic',
        source: 'renderer',
        timestamp: 1,
        accepted: true,
      },
    }

    const parsed = responseSchema.parse(raw)

    expect(parsed.change).toMatchObject({ accepted: true })
  })
})
