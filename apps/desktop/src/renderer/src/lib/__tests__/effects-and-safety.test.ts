import { beforeEach, describe, expect, it, vi } from 'vitest'
import { resolveEffectQuality } from '../effect-quality'
import { parseMatchEffectToolResult } from '../match-effect'
import { readLocalStorage, writeLocalStorage } from '../safe-local-storage'
import { isSensitiveArgName, maskSensitiveArgs } from '../sensitive-tool-arg'

describe('sensitive-tool-arg', () => {
  it('detecta chaves sensíveis por regex', () => {
    expect(isSensitiveArgName('apiKey')).toBe(true)
    expect(isSensitiveArgName('secret_token')).toBe(true)
    expect(isSensitiveArgName('userPassword')).toBe(true)
    expect(isSensitiveArgName('credentialInfo')).toBe(true)
    expect(isSensitiveArgName('message')).toBe(false)
    expect(isSensitiveArgName('query')).toBe(false)
  })

  it('mascara objetos que contêm chaves sensíveis', () => {
    const raw = {
      username: 'mika_fan',
      apiKey: 'sk-1234567890',
      password: 'supersecretpassword',
      count: 42,
    }
    const masked = maskSensitiveArgs(raw) as Record<string, unknown>
    expect(masked.username).toBe('mika_fan')
    expect(masked.apiKey).toBe('***')
    expect(masked.password).toBe('***')
    expect(masked.count).toBe(42)

    // Primitivos e arrays retornam intactos
    expect(maskSensitiveArgs('string')).toBe('string')
    expect(maskSensitiveArgs([1, 2, 3])).toEqual([1, 2, 3])
  })
})

describe('safe-local-storage', () => {
  const store = new Map<string, string>()

  beforeEach(() => {
    store.clear()
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value)
      },
    })
  })

  it('lê e escreve com segurança no localStorage', () => {
    const ok = writeLocalStorage('test_key', 'val_123')
    expect(ok).toBe(true)
    const read = readLocalStorage('test_key')
    expect(read).toBe('val_123')
  })

  it('retorna null para chaves inexistentes', () => {
    expect(readLocalStorage('non_existent_key')).toBeNull()
  })

  it('captura falhas e retorna seguro se localStorage lançar', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('Access denied')
      },
      setItem: () => {
        throw new Error('Quota exceeded')
      },
    })
    expect(readLocalStorage('key')).toBeNull()
    expect(writeLocalStorage('key', 'val')).toBe(false)
  })
})

describe('match-effect', () => {
  it('faz parse de resultados válidos de ferramenta de efeito', () => {
    const valid = {
      success: true,
      data: {
        triggered: true,
        effect: 'sakura',
        type: 'show',
      },
    }
    expect(parseMatchEffectToolResult(valid)).toEqual({
      effect: 'sakura',
      mode: 'show',
    })

    const toggle = {
      success: true,
      data: {
        triggered: true,
        effect: 'matrix',
        type: 'toggle',
      },
    }
    expect(parseMatchEffectToolResult(toggle)).toEqual({
      effect: 'matrix',
      mode: 'toggle',
    })
  })

  it('rejeita envelopes inválidos ou efeitos desconhecidos', () => {
    expect(parseMatchEffectToolResult(null)).toBeNull()
    expect(parseMatchEffectToolResult({ success: false })).toBeNull()
    expect(parseMatchEffectToolResult({ success: true, data: { triggered: false } })).toBeNull()
    expect(parseMatchEffectToolResult({ success: true, data: { triggered: true, effect: 'unknown' } })).toBeNull()
  })
})

describe('effect-quality', () => {
  it('pausa efeitos quando oculto', () => {
    expect(
      resolveEffectQuality({
        hidden: true,
        focused: true,
        onBattery: false,
      }),
    ).toBe('paused')
  })

  it('reduz ou pausa com reducedMotion', () => {
    expect(
      resolveEffectQuality({
        hidden: false,
        focused: true,
        onBattery: false,
        reducedMotion: true,
        speaking: false,
      }),
    ).toBe('paused')

    expect(
      resolveEffectQuality({
        hidden: false,
        focused: true,
        onBattery: false,
        reducedMotion: true,
        speaking: true,
      }),
    ).toBe('reduced')
  })

  it('resolve para full quando focado na tomada, ou reduced na bateria', () => {
    expect(
      resolveEffectQuality({
        hidden: false,
        focused: true,
        onBattery: false,
      }),
    ).toBe('full')

    expect(
      resolveEffectQuality({
        hidden: false,
        focused: true,
        onBattery: true,
      }),
    ).toBe('reduced')
  })

  it('resolve para eco quando em segundo plano sem fala', () => {
    expect(
      resolveEffectQuality({
        hidden: false,
        focused: false,
        onBattery: false,
        speaking: false,
      }),
    ).toBe('eco')
  })
})
