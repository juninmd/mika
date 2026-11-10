import { describe, expect, it } from 'vitest'
import { readPath, toErrorMessage, typedEntries, writePathImmutable } from '../../src/utils/object'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xb1ec7)

function randomString(length: number): string {
  let result = ''
  const characters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?\\/'
  const charactersLength = characters.length
  for (let i = 0; i < length; i++) {
    result += characters.charAt(Math.floor(random() * charactersLength))
  }
  return result
}

function randomKeyArray(depth: number): string[] {
  const keys = []
  for (let i = 0; i < depth; i++) {
    let k = randomString(Math.floor(random() * 10) + 1)
    while (['__proto__', 'constructor', 'prototype'].includes(k)) k += 'x'
    keys.push(k)
  }
  return keys
}

function randomErrorObject(): unknown {
  const types = [
    () => new Error(randomString(20)),
    () => randomString(50),
    () => null,
    () => undefined,
    () => 42,
    () => ({ message: randomString(20) }),
    () => ({ error: randomString(20) }),
    () => ({ error: { message: randomString(20) } }),
    () => ({ statusCode: 404 }),
    () => ({ data: { details: randomString(50) } }),
    () => {
      const obj: Record<string, unknown> = {}
      obj.self = obj
      return obj
    },
  ]
  return types[Math.floor(random() * types.length)]()
}

describe('Object Utilities - Fuzzy Tests', () => {
  it('toErrorMessage should never crash on random or circular objects', () => {
    for (let i = 0; i < 1000; i++) {
      const err = randomErrorObject()
      const msg = toErrorMessage(err)
      expect(typeof msg).toBe('string')
    }
  })

  it('writePathImmutable and readPath should handle arbitrary valid paths', () => {
    for (let i = 0; i < 1000; i++) {
      const keys = randomKeyArray(Math.floor(random() * 5) + 1)
      const val = randomString(10)

      const newObj = writePathImmutable({}, keys, val)
      const readVal = readPath(newObj, keys)

      expect(readVal).toBe(val)
    }
  })

  it('typedEntries should extract keys and values exactly matching Object.entries', () => {
    for (let i = 0; i < 500; i++) {
      const obj: Record<string, unknown> = {}
      const keys = randomKeyArray(Math.floor(random() * 20))

      for (const k of keys) {
        obj[k] = randomString(5)
      }

      const standardEntries = Object.entries(obj)
      const customEntries = typedEntries(obj)

      expect(customEntries).toEqual(standardEntries)
    }
  })
})
