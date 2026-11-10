import { describe, expect, it } from 'vitest'
import { cn } from '../cn'

// `cn` composes clsx (conditional class joining) with tailwind-merge (conflict resolution):
// the last conflicting Tailwind utility wins instead of both being emitted.
describe('cn', () => {
  it('joins plain string class names with a space', () => {
    expect(cn('a', 'b', 'c')).toBe('a b c')
  })

  it('drops falsy values — undefined, null, false, empty string', () => {
    expect(cn('a', undefined, null, false, '', 'b')).toBe('a b')
  })

  it('applies object syntax, keeping only truthy keys', () => {
    expect(cn({ a: true, b: false, c: 1 > 0 })).toBe('a c')
  })

  it('flattens arrays of class values', () => {
    expect(cn(['a', 'b'], 'c')).toBe('a b c')
  })

  it('resolves conflicting Tailwind utilities — last one wins, not both', () => {
    expect(cn('p-2', 'p-4')).toBe('p-4')
  })

  // StatusPill rendered at the parent's size: the colour class erased the custom size token.
  it('treats the theme sizes text-micro/text-caption as font sizes, not colours', () => {
    expect(cn('text-micro font-black', 'text-ok')).toBe('text-micro font-black text-ok')
    expect(cn('text-caption', 'text-text-muted')).toBe('text-caption text-text-muted')
    expect(cn('text-micro', 'text-sm')).toBe('text-sm')
  })

  it('keeps non-conflicting utilities alongside a resolved conflict', () => {
    expect(cn('text-sm', 'font-bold', 'text-lg')).toBe('font-bold text-lg')
  })

  it('with no inputs returns an empty string', () => {
    expect(cn()).toBe('')
  })
})
