import { describe, expect, it } from 'vitest'
import { findSetupProblems } from '../setup-check.mts'

describe('setup check', () => {
  it('names the Linux libraries pkg-config cannot find', () => {
    const has = (cmd: string, args: string[]) =>
      cmd !== 'pkg-config' || args[0] === '--version' || args[1] !== 'webkit2gtk-4.1'
    // rustc fake too: the real one behind the rustup shim took >5s on CI.
    expect(findSetupProblems('linux', has, () => 'rustc 99.0.0').join('\n')).toContain('webkit2gtk-4.1')
  })

  it('reports a missing Rust toolchain on every platform', () => {
    expect(findSetupProblems('win32', () => false).join('\n')).toContain('Rust ausente')
  })

  it('does not probe pkg-config outside Linux', () => {
    expect(
      findSetupProblems(
        'darwin',
        () => true,
        () => 'rustc 99.0.0',
      ).join('\n'),
    ).not.toContain('pkg-config')
  })
})
