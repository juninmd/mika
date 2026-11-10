import { describe, expect, it } from 'vitest'
import { getByPath } from '../get-by-path'

// A literal key containing a dot must win over the nested-path interpretation — that is the
// whole reason this helper exists instead of a one-line `path.split('.').reduce(...)`.
describe('getByPath', () => {
  it('reads a nested path across multiple levels', () => {
    expect(getByPath({ a: { b: { c: 42 } } }, 'a.b.c')).toBe(42)
  })

  it('reads a top-level key with no dot', () => {
    expect(getByPath({ a: 1 }, 'a')).toBe(1)
  })

  it('a literal key containing a dot wins over the nested path', () => {
    const obj = { 'a.b': 'literal', a: { b: 'nested' } }
    expect(getByPath(obj, 'a.b')).toBe('literal')
  })

  it('falls back to nested traversal when no literal dotted key exists', () => {
    expect(getByPath({ a: { b: 'nested' } }, 'a.b')).toBe('nested')
  })

  it('returns undefined when an intermediate segment is missing', () => {
    expect(getByPath({ a: {} }, 'a.b.c')).toBeUndefined()
  })

  it('returns undefined when an intermediate segment is a primitive, not an object', () => {
    expect(getByPath({ a: 5 }, 'a.b')).toBeUndefined()
  })

  it('returns undefined when an intermediate segment is null', () => {
    expect(getByPath({ a: null }, 'a.b')).toBeUndefined()
  })

  it('returns the value even when it is falsy (0, "", false)', () => {
    expect(getByPath({ a: { b: 0 } }, 'a.b')).toBe(0)
    expect(getByPath({ a: { b: '' } }, 'a.b')).toBe('')
    expect(getByPath({ a: { b: false } }, 'a.b')).toBe(false)
  })

  it('returns undefined for an empty object with any path', () => {
    expect(getByPath({}, 'a.b')).toBeUndefined()
  })
})
