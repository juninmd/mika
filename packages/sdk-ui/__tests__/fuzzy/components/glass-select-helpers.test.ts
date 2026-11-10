import { describe, expect, it } from 'vitest'
import { seeded } from '../../../../sdk/__tests__/fuzzy/prng'
import {
  computePosition,
  containerClass,
  nextIndexFor,
  readCurrentValue,
  rendersAsCards,
  triggerClass,
} from '../../../src/components/glass-select-helpers'

const rand = seeded(5678)

describe('Glass Select Helpers - Fuzzy Tests', () => {
  // biome-ignore lint/complexity/noExcessiveCognitiveComplexity: fuzzy testing requires multiple branches
  it('rendersAsCards should resolve correctly based on variant, searchable, hideLabel and options length', () => {
    for (let i = 0; i < 500; i++) {
      const variant = rand() < 0.33 ? 'auto' : rand() < 0.66 ? 'cards' : 'dropdown'
      const searchable = rand() > 0.5
      const hideLabel = rand() > 0.5
      const optionsLength = Math.floor(rand() * 10)
      const options = Array.from({ length: optionsLength }, () => ({ label: 'l', value: 'v' }))

      const result = rendersAsCards({ variant, searchable, hideLabel, options, label: 'test' })

      if (variant === 'cards') {
        expect(result).toBe(true)
      } else if (variant === 'dropdown') {
        expect(result).toBe(false)
      } else {
        // variant auto
        const expected = !searchable && !hideLabel && optionsLength >= 2 && optionsLength <= 6
        expect(result).toBe(expected)
      }
    }
  })

  it('computePosition should return valid position values and prevent out-of-bounds', () => {
    for (let i = 0; i < 1000; i++) {
      // Mock window size
      global.window = global.window || {}
      global.window = Object.create(global.window)
      Object.defineProperty(window, 'innerHeight', { value: Math.floor(rand() * 1000) + 500 })
      Object.defineProperty(window, 'innerWidth', { value: Math.floor(rand() * 1000) + 500 })

      const top = Math.floor(rand() * window.innerHeight)
      const left = Math.floor(rand() * window.innerWidth)
      const height = Math.floor(rand() * 100) + 20
      const width = Math.floor(rand() * 500) + 50

      const rect = {
        top,
        bottom: top + height,
        left,
        right: left + width,
        width,
        height,
      } as DOMRect

      const panelHeight = Math.floor(rand() * 600) + 100

      const position = computePosition(rect, panelHeight)

      expect(typeof position.left).toBe('number')
      expect(typeof position.top).toBe('number')
      expect(typeof position.width).toBe('number')
      expect(typeof position.listMaxHeight).toBe('number')

      expect(position.left).toBeGreaterThanOrEqual(8) // VIEWPORT_MARGIN
      expect(position.top).toBeGreaterThanOrEqual(8)
      expect(position.width).toBeGreaterThanOrEqual(0)
    }
  })

  it('readCurrentValue should correctly resolve from config/configPath or value fallback', () => {
    for (let i = 0; i < 500; i++) {
      const configPath = rand() > 0.5 ? 'test.path' : undefined
      const config = rand() > 0.5 ? { test: { path: rand() > 0.5 ? 'config-val' : 123 } } : undefined
      const value = rand() > 0.5 ? 'fallback-val' : undefined

      const result = readCurrentValue({ configPath, config, value, label: 'test', options: [] })

      expect(typeof result).toBe('string')
    }
  })

  it('containerClass should append disabled class appropriately', () => {
    for (let i = 0; i < 100; i++) {
      const disabled = rand() > 0.5
      const result = containerClass(disabled)
      expect(typeof result).toBe('string')
      if (disabled) expect(result).toContain('opacity-50')
    }
  })

  it('triggerClass should append isOpen classes appropriately', () => {
    for (let i = 0; i < 100; i++) {
      const isOpen = rand() > 0.5
      const result = triggerClass(isOpen)
      expect(typeof result).toBe('string')
      if (isOpen) expect(result).toContain('ring-2')
    }
  })

  it('nextIndexFor should navigate through items and handle boundaries safely', () => {
    const keys = ['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', 'Escape', 'A', ' ']

    for (let i = 0; i < 500; i++) {
      const lastIndex = Math.floor(rand() * 20) + 1
      const current = Math.floor(rand() * (lastIndex + 1))
      const key = keys[Math.floor(rand() * keys.length)] as string

      const result = nextIndexFor(key, current, lastIndex)

      if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(key)) {
        expect(result).toBeGreaterThanOrEqual(0)
        expect(result).toBeLessThanOrEqual(lastIndex)
      } else {
        expect(result).toBeNull()
      }
    }
  })
})
