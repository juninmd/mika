import { describe, expect, it } from 'vitest'
import { nextTabIndex, rovingTabIndex } from '../sub-tab-navigation'

describe('nextTabIndex (WAI-ARIA tablist keys)', () => {
  it('Arrow keys wrap around both ends', () => {
    expect(nextTabIndex('ArrowRight', 0, 3)).toBe(1)
    expect(nextTabIndex('ArrowRight', 2, 3)).toBe(0)
    expect(nextTabIndex('ArrowLeft', 0, 3)).toBe(2)
    expect(nextTabIndex('ArrowLeft', 2, 3)).toBe(1)
  })

  it('Home/End jump to the ends', () => {
    expect(nextTabIndex('Home', 2, 4)).toBe(0)
    expect(nextTabIndex('End', 0, 4)).toBe(3)
  })

  it('with focus outside the tabs an arrow lands on the first tab', () => {
    expect(nextTabIndex('ArrowRight', -1, 3)).toBe(0)
    expect(nextTabIndex('ArrowLeft', -1, 3)).toBe(0)
  })

  it('ignores other keys and empty lists', () => {
    expect(nextTabIndex('Enter', 0, 3)).toBeNull()
    expect(nextTabIndex('ArrowRight', 0, 0)).toBeNull()
  })
})

describe('rovingTabIndex', () => {
  // Only one tab is in the Tab order; if the active tab is not in this section, the first one takes the slot.
  it('active tab is 0, the rest -1', () => {
    expect(['a', 'b', 'c'].map((id) => rovingTabIndex(id, 'b', ['a', 'b', 'c']))).toEqual([-1, 0, -1])
  })

  it('falls back to the first tab when the active one is not part of the list', () => {
    expect(['a', 'b'].map((id) => rovingTabIndex(id, 'zzz', ['a', 'b']))).toEqual([0, -1])
    expect(['a', 'b'].map((id) => rovingTabIndex(id, null, ['a', 'b']))).toEqual([0, -1])
  })
})
