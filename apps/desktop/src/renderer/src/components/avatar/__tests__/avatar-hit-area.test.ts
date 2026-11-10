import { describe, expect, it } from 'vitest'
import { isHeadHitArea } from '../avatar-hit-area'

describe('isHeadHitArea', () => {
  it('a click near the top-center of the canvas hits the head', () => {
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.1 })).toBe(true)
  })

  it('a click near the bottom (feet) misses the head', () => {
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.9 })).toBe(false)
  })

  it('a click on the shoulders/chest band, below the head, misses', () => {
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.4 })).toBe(false)
  })

  it('a click off to the side of a top-center head misses, even at head height', () => {
    expect(isHeadHitArea({ xFrac: 0.05, yFrac: 0.1 })).toBe(false)
    expect(isHeadHitArea({ xFrac: 0.95, yFrac: 0.1 })).toBe(false)
  })

  it('a click above the model entirely (empty canvas space) misses', () => {
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0 })).toBe(false)
  })

  it('the head band edges are inclusive: exactly on HEAD_X_MIN/MAX or HEAD_Y_MIN/MAX hits', () => {
    expect(isHeadHitArea({ xFrac: 0.3, yFrac: 0.1 })).toBe(true)
    expect(isHeadHitArea({ xFrac: 0.7, yFrac: 0.1 })).toBe(true)
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.04 })).toBe(true)
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.24 })).toBe(true)
  })

  it('one step outside the head band edges misses', () => {
    expect(isHeadHitArea({ xFrac: 0.29, yFrac: 0.1 })).toBe(false)
    expect(isHeadHitArea({ xFrac: 0.71, yFrac: 0.1 })).toBe(false)
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.03 })).toBe(false)
    expect(isHeadHitArea({ xFrac: 0.5, yFrac: 0.25 })).toBe(false)
  })
})
