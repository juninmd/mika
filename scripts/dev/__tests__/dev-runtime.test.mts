import { describe, expect, it } from 'vitest'
import { getDevRuntimeError } from '../node-runtime-guard.mts'

describe('desktop dev runtime', () => {
  it('blocks the broken Windows Node.js 24.18.0 patch before it can crash Vite', () => {
    expect(getDevRuntimeError('24.18.0', 'win32')).toContain('Node.js 24.19+ LTS')
  })

  it.each(['24.19.0', '24.19.1', '24.20.0'] as const)('accepts working Node.js LTS %s', (version) => {
    expect(getDevRuntimeError(version, 'win32')).toBeNull()
  })

  it.each(['linux', 'darwin', 'win32'] as const)('blocks Node.js older than the engines floor on %s', (platform) => {
    expect(getDevRuntimeError('22.22.0', platform)).toContain('Node.js 24.19+ LTS')
    expect(getDevRuntimeError('24.18.1', platform)).toContain('Node.js 24.19+ LTS')
  })

  it('does not apply the Windows-only 24.18.0 crash message on other platforms', () => {
    expect(getDevRuntimeError('24.18.0', 'linux')).toContain('engines')
  })
})
