import { describe, expect, it } from 'vitest'
import { capitalize, formatFixed } from '../locale-text'

describe('formatFixed', () => {
  // `Number#toFixed` always prints a dot, so a German/Russian UI showed "0.50" next to "0,50" elsewhere.
  it('keeps exactly `digits` decimals with the locale separator', () => {
    expect(formatFixed(0.5, 2, 'en-US')).toBe('0.50')
    expect(formatFixed(0.5, 2, 'de-DE')).toBe('0,50')
    expect(formatFixed(1234.5, 1, 'de-DE')).toBe('1.234,5')
    expect(formatFixed(3, 0, 'en-US')).toBe('3')
  })
})

describe('capitalize', () => {
  it('upper-cases the first character by locale rules (Turkish dotted i), not by UTF-16 unit', () => {
    expect(capitalize('mage', 'en-US')).toBe('Mage')
    expect(capitalize('istanbul', 'tr-TR')).toBe('İstanbul')
    expect(capitalize('\u{1F600}x', 'en-US')).toBe('\u{1F600}x')
    expect(capitalize('', 'en-US')).toBe('')
  })
})
