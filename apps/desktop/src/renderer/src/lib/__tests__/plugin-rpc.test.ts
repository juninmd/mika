import { describe, expect, it } from 'vitest'
import { unwrapRpcData } from '../plugin-rpc'

describe('unwrapRpcData', () => {
  // The Rust gamer-companion handlers return the bare data; the transport envelope is already gone by here.
  it('passes bare handler data through untouched', () => {
    const session = { id: 's1', gameName: 'Hades' }
    expect(unwrapRpcData(session)).toBe(session)
    expect(unwrapRpcData(null)).toBeNull()
    expect(unwrapRpcData({ steamFound: false, source: 'installed', games: [] })).toEqual({
      steamFound: false,
      source: 'installed',
      games: [],
    })
  })

  // The TS-era handlers (and person-catalog still) answer `{ success, data }`.
  it('unwraps a business envelope', () => {
    expect(unwrapRpcData({ success: true, data: { art: 'x' } })).toEqual({ art: 'x' })
    expect(unwrapRpcData({ success: true })).toBeUndefined()
  })

  it('throws the envelope error text on a failed business envelope', () => {
    expect(() => unwrapRpcData({ success: false, error: 'appId inválido' })).toThrow('appId inválido')
    expect(() => unwrapRpcData({ success: false })).toThrow()
  })
})
