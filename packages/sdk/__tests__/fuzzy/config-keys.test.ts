import { describe, expect, it } from 'vitest'
import { isCredentialConfigPath, SENSITIVE_CONFIG_PATHS } from '../../src/contracts/schemas/config-keys.enum'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xc0f16)

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-.'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(random() * chars.length))
  }
  return result
}

describe('Config Keys - Fuzzy Tests', () => {
  it('isCredentialConfigPath should identify known sensitive suffixes', () => {
    const sensitiveSuffixes = ['Token', 'Secret', 'Password', 'Key', 'Baseurl', 'Endpoint', 'Credential', 'Dsn']
    for (let i = 0; i < 500; i++) {
      const prefix = randomString(10)
      const suffix = sensitiveSuffixes[Math.floor(random() * sensitiveSuffixes.length)]
      const path = `${prefix}${suffix}`
      expect(isCredentialConfigPath(path)).toBe(true)
    }
  })

  it('SENSITIVE_CONFIG_PATHS should all be identified as credentials', () => {
    SENSITIVE_CONFIG_PATHS.forEach((path) => {
      expect(isCredentialConfigPath(path)).toBe(true)
    })
  })
})
