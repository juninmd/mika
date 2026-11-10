import { describe, expect, it } from 'vitest'
import { parseNumberDraft } from '../components/aurora-field-helpers'

describe('parseNumberDraft', () => {
  // Regression: a cleared number field was saved as Number('') === 0, silently overwriting e.g. user.age.
  it.each(['', '   ', '\t'])('treats blank %j as "nothing to save", never as 0', (draft) => {
    expect(parseNumberDraft(draft)).toBeNull()
  })

  it.each(['abc', '12abc', 'Infinity', '-Infinity', 'NaN'])('rejects non-finite input %j', (draft) => {
    expect(parseNumberDraft(draft)).toBeNull()
  })

  it.each([
    ['30', 30],
    ['0', 0],
    ['-4', -4],
    ['2.5', 2.5],
    [' 7 ', 7],
  ])('parses %j as %d (a typed zero is still a value)', (draft, value) => {
    expect(parseNumberDraft(draft)).toBe(value)
  })
})
