import { isValidElement } from 'react'
import { describe, expect, it } from 'vitest'
import { highlightText } from '../highlight-text'

// Mirrors ChatHistoryPanel: the regex is built from the trimmed query, the raw query is passed along.
const regexFor = (query: string) => new RegExp(`(${query.trim().replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')

const marked = (node: unknown) =>
  (Array.isArray(node) ? node : [node]).filter(isValidElement).map((el) => (el.props as { children: string }).children)

describe('highlightText', () => {
  it('marks every case-insensitive match', () => {
    expect(marked(highlightText('Hello hello', 'hello', regexFor('hello')))).toEqual(['Hello', 'hello'])
  })

  it('still marks matches while the user is mid-typing a second word (trailing space in the query)', () => {
    // The panel filters on the trimmed query, so this message is listed; it must also be highlighted.
    expect(marked(highlightText('say hello there', 'hello ', regexFor('hello ')))).toEqual(['hello'])
  })

  it('returns the text untouched without a regex', () => {
    expect(highlightText('hello', 'hello', null)).toBe('hello')
  })
})
