import { Sparkles } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import { type PaletteItem, sharedGroupDescription } from '../command-palette-items'

const item = (id: string, description?: string): PaletteItem => ({
  id,
  label: id,
  description,
  icon: Sparkles,
  group: 'g',
  onSelect: () => {},
})

describe('sharedGroupDescription', () => {
  it('returns the description a whole group repeats, so the header can say it once', () => {
    const effects = ['matrix', 'stars', 'sakura'].map((id) => item(id, 'Toggles on screen, bypassing the model'))
    expect(sharedGroupDescription(effects)).toBe('Toggles on screen, bypassing the model')
  })

  it('keeps per-item descriptions when they differ', () => {
    expect(sharedGroupDescription([item('a', 'one'), item('b', 'two'), item('c', 'one')])).toBeUndefined()
  })

  it('does not collapse a pair: two lines are not repetition worth a header note', () => {
    expect(sharedGroupDescription([item('a', 'same'), item('b', 'same')])).toBeUndefined()
  })

  it('ignores groups with no description', () => {
    expect(sharedGroupDescription([item('a'), item('b'), item('c')])).toBeUndefined()
  })
})
