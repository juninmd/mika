import { describe, expect, it } from 'vitest'
import { buildInputClass, buildWrapperClass, readInitial } from '../aurora-field-helpers'

describe('buildInputClass', () => {
  it('shows the hover border style when unfocused', () => {
    expect(buildInputClass(false)).toContain('hover:border-border-strong')
  })

  it('drops the hover style while focused', () => {
    expect(buildInputClass(true)).not.toContain('hover:border-border-strong')
  })
})

describe('readInitial', () => {
  it('reads from config via configPath in the config-bound mode', () => {
    const result = readInitial({
      label: 'x',
      config: { a: { b: 'from-config' } },
      configPath: 'a.b',
    })
    expect(result).toBe('from-config')
  })

  it('prefers the controlled value prop when onChange is provided, even with a configPath set', () => {
    const result = readInitial({
      label: 'x',
      config: { a: { b: 'from-config' } },
      configPath: 'a.b',
      value: 'controlled',
      onChange: () => {},
    })
    expect(result).toBe('controlled')
  })

  it('falls back to the plain value prop when config/configPath are absent', () => {
    expect(readInitial({ label: 'x', value: 'plain' })).toBe('plain')
  })

  it('coerces a non-string config value to string', () => {
    const result = readInitial({ label: 'x', config: { count: 7 }, configPath: 'count' })
    expect(result).toBe('7')
  })

  it('returns an empty string when nothing resolves', () => {
    expect(readInitial({ label: 'x' })).toBe('')
  })
})

describe('buildWrapperClass', () => {
  it('adds the disabled opacity modifier only when disabled', () => {
    expect(buildWrapperClass(true)).toContain('opacity-50')
    expect(buildWrapperClass(false)).not.toContain('opacity-50')
  })
})
