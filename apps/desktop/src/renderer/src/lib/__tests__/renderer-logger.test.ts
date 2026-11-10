import { afterEach, describe, expect, it, vi } from 'vitest'
import { rendererLog } from '../renderer-logger'

afterEach(() => {
  vi.restoreAllMocks()
})

describe('rendererLog', () => {
  it('serializa o payload dentro da mensagem, e não como segundo argumento', () => {
    const spy = vi.spyOn(console, 'info').mockImplementation(() => {})

    rendererLog.info('[TTS] Audio payload received', { length: 14208, mimeType: 'audio/mpeg' })

    expect(spy).toHaveBeenCalledTimes(1)
    const [line, ...extraArgs] = spy.mock.calls[0] as string[]
    expect(extraArgs).toHaveLength(0)
    expect(line).toContain('"length":14208')
    expect(line).toContain('"mimeType":"audio/mpeg"')
    expect(line).not.toContain('[object Object]')
  })

  it('identifica o call site real, não o próprio logger', () => {
    const spy = vi.spyOn(console, 'warn').mockImplementation(() => {})

    rendererLog.warn('algo')

    const [line] = spy.mock.calls[0] as string[]
    expect(line).toContain('renderer-logger.test.ts:')
    expect(line).not.toContain('renderer-logger.ts:')
  })

  it('não deixa referência circular nem getter quebrado derrubar a superfície', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const circular: Record<string, unknown> = { name: 'loop' }
    circular.self = circular

    expect(() => rendererLog.error('ciclo', circular)).not.toThrow()
    expect((spy.mock.calls[0] as string[])[0]).toContain('[circular]')
  })

  it('achata Error para nome e mensagem em vez de `{}`', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})

    rendererLog.error('falhou', new TypeError('sem sinkId'))

    expect((spy.mock.calls[0] as string[])[0]).toContain('TypeError: sem sinkId')
  })
})
