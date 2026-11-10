import { describe, expect, it } from 'vitest'
import { jsonFailureReason, redactArgs, redactLogMeta, summarizeResult } from '../../src/utils/redaction'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0x5ed4c7)

function randomString(length: number): string {
  let result = ''
  const characters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
  const charactersLength = characters.length
  for (let i = 0; i < length; i++) {
    result += characters.charAt(Math.floor(random() * charactersLength))
  }
  return result
}

function generateRandomArray(depth: number, maxDepth: number): unknown[] {
  const arrLength = Math.floor(random() * 5)
  const arr = []
  for (let i = 0; i < arrLength; i++) {
    arr.push(generateRandomObject(depth + 1, maxDepth))
  }
  return arr
}

function generateRandomRecord(depth: number, maxDepth: number): Record<string, unknown> {
  const objLength = Math.floor(random() * 5)
  const obj: Record<string, unknown> = {}
  for (let i = 0; i < objLength; i++) {
    let key = randomString(Math.floor(random() * 20) + 1)
    const rand = random()
    if (rand > 0.8) {
      key += 'password'
    } else if (rand > 0.6) {
      key += 'history'
    }
    obj[key] = generateRandomObject(depth + 1, maxDepth)
  }
  return obj
}

function generateRandomObject(depth: number, maxDepth: number): unknown {
  if (depth >= maxDepth) {
    return randomString(10)
  }
  const type = Math.floor(random() * 6)
  if (type === 0) return randomString(Math.floor(random() * 300)) // Can be > 200
  if (type === 1) return random() * 1000
  if (type === 2) return random() > 0.5
  if (type === 3) return null
  if (type === 4) return generateRandomArray(depth, maxDepth)
  return generateRandomRecord(depth, maxDepth)
}

describe('Redaction - Fuzzy Tests', () => {
  it('should not throw errors when redacting random deep objects in redactArgs', () => {
    for (let i = 0; i < 100; i++) {
      const input = generateRandomObject(0, 10) as Record<string, unknown>
      const result = redactArgs(input)
      expect(result).toBeDefined()
    }
  })

  it('should not throw errors when redacting random deep objects in redactLogMeta', () => {
    for (let i = 0; i < 100; i++) {
      const input = generateRandomObject(0, 10) as Record<string, unknown>
      const result = redactLogMeta(input)
      expect(result).toBeDefined()
    }
  })

  it('should not throw errors when redacting extremely deeply nested objects', () => {
    const deepObject: Record<string, unknown> = {}
    let current = deepObject
    for (let i = 0; i < 20; i++) {
      current.nested = {}
      current = current.nested as Record<string, unknown>
    }
    current.secret = 'should_be_redacted'

    expect(() => redactArgs(deepObject)).not.toThrow()
  })

  it('should safely summarize result with random objects', () => {
    for (let i = 0; i < 100; i++) {
      const input = generateRandomObject(0, 5)
      const result = summarizeResult(input)
      expect(result).toBeDefined()
    }
  })

  it('should safely process json failure reasons with random errors', () => {
    for (let i = 0; i < 100; i++) {
      const err = new Error(randomString(100))
      const result = jsonFailureReason(err)
      expect(typeof result).toBe('string')

      const nonErr = generateRandomObject(0, 3)
      const result2 = jsonFailureReason(nonErr)
      expect(typeof result2).toBe('string')
    }
  })
})
