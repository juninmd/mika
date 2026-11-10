import { describe, expect, it } from 'vitest'
import {
  desktopPluginViewRequestSchema,
  emptyPayloadSchema,
  voidResponseSchema,
} from '../../src/contracts/desktop-ipc-core-schemas'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0x1bc)

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(random() * chars.length))
  }
  return result
}

describe('Desktop IPC Core Schemas - Fuzzy Tests', () => {
  it('emptyPayloadSchema should accept undefined', () => {
    expect(emptyPayloadSchema.safeParse(undefined).success).toBe(true)
    expect(emptyPayloadSchema.safeParse({}).success).toBe(false)
    expect(emptyPayloadSchema.safeParse(null).success).toBe(false)
  })

  it('voidResponseSchema should accept undefined', () => {
    expect(voidResponseSchema.safeParse(undefined).success).toBe(true)
    expect(voidResponseSchema.safeParse({}).success).toBe(false)
    expect(voidResponseSchema.safeParse(null).success).toBe(false)
  })

  it('desktopPluginViewRequestSchema should validate objects with min length 1 correctly', () => {
    for (let i = 0; i < 500; i++) {
      const pluginId = randomString(Math.floor(random() * 50) + 1)
      const viewId = randomString(Math.floor(random() * 50) + 1)

      const result = desktopPluginViewRequestSchema.safeParse({ pluginId, viewId })
      expect(result.success).toBe(true)
    }
    expect(desktopPluginViewRequestSchema.safeParse({ pluginId: '', viewId: '' }).success).toBe(false)
  })
})
