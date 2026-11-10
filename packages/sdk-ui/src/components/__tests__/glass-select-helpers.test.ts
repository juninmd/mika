import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  computePosition,
  containerClass,
  ESTIMATED_PANEL_HEIGHT,
  nextIndexFor,
  readCurrentValue,
  rendersAsCards,
  triggerClass,
} from '../glass-select-helpers'

function stubWindow(innerWidth: number, innerHeight: number) {
  vi.stubGlobal('window', { innerWidth, innerHeight })
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('computePosition', () => {
  it('opens downward when there is enough space below the trigger', () => {
    stubWindow(1024, 768)
    const rect = { top: 100, bottom: 130, left: 50, width: 200 } as DOMRect
    const pos = computePosition(rect, ESTIMATED_PANEL_HEIGHT)
    expect(pos.top).toBeGreaterThan(rect.bottom)
    expect(pos.left).toBe(50)
    expect(pos.width).toBe(200)
  })

  it('flips upward when space below is smaller than space above', () => {
    stubWindow(1024, 400)
    const rect = { top: 380, bottom: 400, left: 50, width: 200 } as DOMRect
    const pos = computePosition(rect, ESTIMATED_PANEL_HEIGHT)
    // Flipped: panel top sits above the trigger, not below it.
    expect(pos.top).toBeLessThan(rect.top)
  })

  it('clamps left within the viewport when the trigger is near the right edge', () => {
    stubWindow(500, 768)
    const rect = { top: 100, bottom: 130, left: 480, width: 200 } as DOMRect
    const pos = computePosition(rect, ESTIMATED_PANEL_HEIGHT)
    expect(pos.left).toBeLessThanOrEqual(500 - 200 - 8)
    expect(pos.left).toBeGreaterThanOrEqual(8)
  })

  it('never returns a list height below the minimum floor', () => {
    stubWindow(1024, 200)
    const rect = { top: 100, bottom: 110, left: 0, width: 100 } as DOMRect
    const pos = computePosition(rect, ESTIMATED_PANEL_HEIGHT)
    expect(pos.listMaxHeight).toBeGreaterThanOrEqual(120)
  })
})

describe('readCurrentValue', () => {
  it('reads from config via configPath when both are provided', () => {
    const result = readCurrentValue({
      label: 'x',
      options: [],
      config: { a: { b: 'nested' } },
      configPath: 'a.b',
    })
    expect(result).toBe('nested')
  })

  it('falls back to the plain value prop when no configPath is set', () => {
    const result = readCurrentValue({ label: 'x', options: [], value: 'plain' })
    expect(result).toBe('plain')
  })

  it('coerces a non-string config value to string', () => {
    const result = readCurrentValue({
      label: 'x',
      options: [],
      config: { count: 3 },
      configPath: 'count',
    })
    expect(result).toBe('3')
  })

  it('returns an empty string when nothing resolves', () => {
    expect(readCurrentValue({ label: 'x', options: [] })).toBe('')
  })
})

describe('containerClass', () => {
  it('adds the disabled opacity modifier only when disabled', () => {
    expect(containerClass(true)).toContain('opacity-50')
    expect(containerClass(false)).not.toContain('opacity-50')
  })
})

describe('triggerClass', () => {
  it('adds the open ring/glow styling only when the panel is open', () => {
    expect(triggerClass(true)).toContain('shadow-[0_0_20px_var(--accent-glow)]')
    expect(triggerClass(false)).not.toContain('shadow-[0_0_20px_var(--accent-glow)]')
  })
})

describe('nextIndexFor', () => {
  it('wraps ArrowDown from the last index back to the first', () => {
    expect(nextIndexFor('ArrowDown', 2, 2)).toBe(0)
  })

  it('advances ArrowDown by one within bounds', () => {
    expect(nextIndexFor('ArrowDown', 0, 2)).toBe(1)
  })

  it('wraps ArrowUp from the first index to the last', () => {
    expect(nextIndexFor('ArrowUp', 0, 2)).toBe(2)
  })

  it('moves ArrowUp back by one within bounds', () => {
    expect(nextIndexFor('ArrowUp', 1, 2)).toBe(0)
  })

  it('jumps to the first index on Home', () => {
    expect(nextIndexFor('Home', 2, 5)).toBe(0)
  })

  it('jumps to the last index on End', () => {
    expect(nextIndexFor('End', 0, 5)).toBe(5)
  })

  it('returns null for keys it does not handle', () => {
    expect(nextIndexFor('Tab', 1, 5)).toBeNull()
  })
})

describe('rendersAsCards', () => {
  const opts = (n: number) => Array.from({ length: n }, (_, i) => ({ label: `o${i}`, value: `${i}` }))

  it('shows a short choice as cards so every option and its description is visible at once', () => {
    expect(rendersAsCards({ label: 'Tom', options: opts(3) })).toBe(true)
    expect(rendersAsCards({ label: 'Tom', options: opts(6) })).toBe(true)
  })

  it('keeps the dropdown for long, searchable or toolbar lists, and for a single option', () => {
    expect(rendersAsCards({ label: 'Modelo', options: opts(7) })).toBe(false)
    expect(rendersAsCards({ label: 'Modelo', options: opts(3), searchable: true })).toBe(false)
    expect(rendersAsCards({ label: 'Nível', options: opts(3), hideLabel: true })).toBe(false)
    expect(rendersAsCards({ label: 'Único', options: opts(1) })).toBe(false)
  })

  it('obeys an explicit variant', () => {
    expect(rendersAsCards({ label: 'x', options: opts(12), variant: 'cards' })).toBe(true)
    expect(rendersAsCards({ label: 'x', options: opts(3), variant: 'dropdown' })).toBe(false)
  })
})
