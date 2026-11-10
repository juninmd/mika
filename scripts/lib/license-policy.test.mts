import { describe, expect, it } from 'vitest'
import { evaluateLicense } from './license-policy.mts'

describe('evaluateLicense', () => {
  it.each([
    ['MIT', true],
    ['MIT OR Apache-2.0', true],
    ['MIT/Apache-2.0', true],
    ['Apache-2.0 / MIT', true],
    ['Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT', true],
    ['(MIT OR Apache-2.0) AND Unicode-3.0', true],
    ['MIT OR Apache-2.0 OR LGPL-2.1-or-later', true],
    ['GPL-3.0-only', false],
    ['AGPL-3.0-or-later', false],
    ['GPL-2.0+', false],
    ['GPL', false],
    ['AGPL', false],
    ['GPL+', false],
    ['LGPL', false],
    ['GPLv3', false],
    ['GPLv2+', false],
    ['AGPLv3', false],
    ['LGPLv3', false],
    ['GPL3', false],
    ['MIT OR GPLv3', true],
    ['NOASSERTION', false],
    ['MIT AND NOASSERTION', false],
    ['MIT OR NOASSERTION', true],
    ['LGPL-2.1-only', false],
    ['MIT AND GPL-3.0-only', false],
    ['MIT AND (GPL-3.0-only OR Apache-2.0)', true],
    ['CC-BY-NC-4.0', false],
    ['SSPL-1.0', false],
    ['Unknown', false],
    ['', false],
  ])('%s -> ok=%s', (expression, ok) => {
    expect(evaluateLicense(expression).ok).toBe(ok)
  })

  it('aceita LGPL só com exceção explícita e a marca como copyleft', () => {
    const verdict = evaluateLicense('LGPL-3.0-only WITH LGPL-3.0-linking-exception')
    expect(verdict.ok).toBe(true)
    expect(verdict.weakCopyleft).toBe(true)
  })

  it('num OR prefere a alternativa sem copyleft', () => {
    const verdict = evaluateLicense('(MPL-2.0 OR Apache-2.0)')
    expect(verdict.ids).toEqual(['Apache-2.0'])
    expect(verdict.weakCopyleft).toBe(false)
  })

  it('marca copyleft fraco quando não há alternativa', () => {
    const verdict = evaluateLicense('EPL-2.0')
    expect(verdict).toMatchObject({ ok: true, weakCopyleft: true })
  })

  it('recusa expressão ilegível em vez de aprovar', () => {
    expect(evaluateLicense('MIT OR (Apache-2.0').ok).toBe(false)
  })
})
