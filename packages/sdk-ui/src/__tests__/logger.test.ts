import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// bridgeState is module-level and shared across createUiLogger() calls — each test needs
// a fresh module instance, otherwise state from one test leaks into the next.
function setElectronApi(api: unknown): void {
  ;(globalThis as { electronAPI?: unknown }).electronAPI = api
}

async function freshLogger() {
  vi.resetModules()
  return import('../logger')
}

describe('createUiLogger — sem bridge (console de fallback)', () => {
  let consoleSpies: Record<'debug' | 'info' | 'warn' | 'error', ReturnType<typeof vi.spyOn>>

  beforeEach(() => {
    setElectronApi(undefined)
    consoleSpies = {
      debug: vi.spyOn(console, 'debug').mockImplementation(() => {}),
      info: vi.spyOn(console, 'info').mockImplementation(() => {}),
      warn: vi.spyOn(console, 'warn').mockImplementation(() => {}),
      error: vi.spyOn(console, 'error').mockImplementation(() => {}),
    }
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('escreve cada nível no console correspondente com o escopo prefixado', async () => {
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('test-scope')
    log.debug('d')
    log.info('i')
    log.warn('w')
    log.error('e')
    expect(consoleSpies.debug).toHaveBeenCalledWith('[test-scope] d')
    expect(consoleSpies.info).toHaveBeenCalledWith('[test-scope] i')
    expect(consoleSpies.warn).toHaveBeenCalledWith('[test-scope] w')
    expect(consoleSpies.error).toHaveBeenCalledWith('[test-scope] e')
  })

  it('anexa meta como JSON quando presente e omite quando vazio', async () => {
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('msg', { a: 1 })
    expect(consoleSpies.info).toHaveBeenCalledWith('[scope] msg {"a":1}')
    consoleSpies.info.mockClear()
    log.info('msg2', {})
    expect(consoleSpies.info).toHaveBeenCalledWith('[scope] msg2')
  })

  it('mensagem vazia e unicode passam sem quebrar a formatação', async () => {
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('')
    expect(consoleSpies.info).toHaveBeenCalledWith('[scope] ')
    log.info('こんにちは 🎮 café')
    expect(consoleSpies.info).toHaveBeenCalledWith('[scope] こんにちは 🎮 café')
  })

  it('mensagem grande é repassada integralmente ao console', async () => {
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    const big = 'x'.repeat(50_000)
    log.warn(big)
    expect(consoleSpies.warn).toHaveBeenCalledWith(`[scope] ${big}`)
  })

  it('child() compõe o escopo com ":" e mantém o nível correto', async () => {
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('parent').child('child')
    log.error('boom')
    expect(consoleSpies.error).toHaveBeenCalledWith('[parent:child] boom')
  })
})

describe('createUiLogger — com bridge do electronAPI', () => {
  afterEach(() => {
    setElectronApi(undefined)
    vi.restoreAllMocks()
  })

  it('bridge vivo (resolve true) recebe as entradas e nunca escreve no console', async () => {
    const bridge = vi.fn().mockResolvedValue(true)
    setElectronApi({ runtime: { log: bridge } })
    const consoleInfo = vi.spyOn(console, 'info').mockImplementation(() => {})
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('first')
    await vi.waitFor(() => expect(bridge).toHaveBeenCalledTimes(1))
    await new Promise((resolve) => setTimeout(resolve, 0))
    log.info('second')
    expect(bridge).toHaveBeenCalledTimes(2)
    expect(consoleInfo).not.toHaveBeenCalled()
  })

  it('the entry that probes a live bridge is delivered once, not again when the queue drains', async () => {
    const bridge = vi.fn().mockResolvedValue(true)
    setElectronApi({ runtime: { log: bridge } })
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('probe')
    log.info('queued')
    await vi.waitFor(() => expect(bridge).toHaveBeenCalledTimes(2))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(bridge.mock.calls.map(([entry]) => entry.msg)).toEqual(['probe', 'queued'])
  })

  it('bridge que resolve false marca o estado absent e cai para o console dali em diante', async () => {
    const bridge = vi.fn().mockResolvedValue(false)
    setElectronApi({ runtime: { log: bridge } })
    const consoleInfo = vi.spyOn(console, 'info').mockImplementation(() => {})
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('probe')
    await vi.waitFor(() => expect(consoleInfo).toHaveBeenCalledWith('[scope] probe'))
    consoleInfo.mockClear()
    log.info('after')
    expect(consoleInfo).toHaveBeenCalledWith('[scope] after')
  })

  it('bridge que rejeita a promessa também cai para console (falha tratada, não propagada)', async () => {
    const bridge = vi.fn().mockRejectedValue(new Error('ipc down'))
    setElectronApi({ runtime: { log: bridge } })
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.error('will retry via console')
    await vi.waitFor(() => expect(consoleError).toHaveBeenCalledWith('[scope] will retry via console'))
  })

  it('sem electronAPI.runtime.log definido, cai direto para o console', async () => {
    setElectronApi({ runtime: {} })
    const consoleDebug = vi.spyOn(console, 'debug').mockImplementation(() => {})
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.debug('no bridge fn')
    expect(consoleDebug).toHaveBeenCalledWith('[scope] no bridge fn')
  })

  it('entradas emitidas durante a sondagem ficam pendentes e são drenadas ao bridge assim que ele resolve vivo', async () => {
    let resolveBridge: (value: boolean) => void = () => {}
    const bridge = vi.fn(
      (_entry: { msg: string }) =>
        new Promise<boolean>((resolve) => {
          resolveBridge = resolve
        }),
    )
    setElectronApi({ runtime: { log: bridge } })
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')
    log.info('probe')
    log.info('queued-1')
    log.info('queued-2')
    expect(bridge).toHaveBeenCalledTimes(1)
    resolveBridge(true)
    await vi.waitFor(() => expect(bridge).toHaveBeenCalledTimes(3))
    expect(bridge.mock.calls.map(([entry]) => entry.msg)).toEqual(['probe', 'queued-1', 'queued-2'])
  })
})
