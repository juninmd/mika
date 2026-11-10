import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { createUiLogger } from '../../src/logger'

const rand = seeded(77)

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(rand() * chars.length))
  }
  return result
}

function randomMeta(): Record<string, unknown> | undefined {
  if (rand() > 0.5) return undefined
  const meta: Record<string, unknown> = {}
  for (let i = 0; i < Math.floor(rand() * 5); i++) {
    meta[randomString(5)] = rand() > 0.5 ? randomString(10) : rand()
  }
  return meta
}

describe('UiLogger - Fuzzy Tests', () => {
  it('should not crash when emitting logs rapidly with random inputs', () => {
    const logger = createUiLogger('test-fuzzy-scope')
    for (let i = 0; i < 1000; i++) {
      const msg = randomString(Math.floor(rand() * 50))
      const meta = randomMeta()
      const type = Math.floor(rand() * 4)

      expect(() => {
        switch (type) {
          case 0:
            logger.debug(msg, meta)
            break
          case 1:
            logger.info(msg, meta)
            break
          case 2:
            logger.warn(msg, meta)
            break
          case 3:
            logger.error(msg, meta)
            break
        }
      }).not.toThrow()
    }
  })

  it('child logger should not crash when emitting logs', () => {
    const logger = createUiLogger('test-fuzzy-scope').child('child-scope')
    for (let i = 0; i < 1000; i++) {
      const msg = randomString(Math.floor(rand() * 50))
      const meta = randomMeta()
      expect(() => logger.info(msg, meta)).not.toThrow()
    }
  })
})
