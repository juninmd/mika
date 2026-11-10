import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { readLocalStorage, writeLocalStorage } from '../safe-local-storage'

function memoryStorage(): Storage {
  const items = new Map<string, string>()
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => {
      items.set(key, String(value))
    },
    removeItem: (key) => {
      items.delete(key)
    },
    clear: () => items.clear(),
    key: (index) => [...items.keys()][index] ?? null,
    get length() {
      return items.size
    },
  }
}

function throwingStorage(): Storage {
  return {
    getItem: () => {
      throw new DOMException('blocked')
    },
    setItem: () => {
      throw new DOMException('blocked')
    },
    removeItem: () => {
      throw new DOMException('blocked')
    },
    clear: () => {
      throw new DOMException('blocked')
    },
    key: () => null,
    length: 0,
  }
}

describe('safe-local-storage', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', memoryStorage())
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('lê e escreve normalmente quando localStorage funciona', () => {
    expect(writeLocalStorage('k', 'v')).toBe(true)
    expect(readLocalStorage('k')).toBe('v')
  })

  it('devolve null quando a chave não existe', () => {
    expect(readLocalStorage('missing')).toBeNull()
  })

  it('não lança e devolve null quando localStorage.getItem lança (modo privado/storage bloqueado)', () => {
    vi.stubGlobal('localStorage', throwingStorage())

    let result: string | null = 'unset'
    expect(() => {
      result = readLocalStorage('k')
    }).not.toThrow()
    expect(result).toBeNull()
  })

  it('não lança e devolve false quando localStorage.setItem lança (modo privado/storage bloqueado)', () => {
    vi.stubGlobal('localStorage', throwingStorage())

    let result: boolean | undefined
    expect(() => {
      result = writeLocalStorage('k', 'v')
    }).not.toThrow()
    expect(result).toBe(false)
  })
})
