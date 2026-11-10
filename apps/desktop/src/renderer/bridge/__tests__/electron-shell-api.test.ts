import { beforeEach, describe, expect, it, vi } from 'vitest'

const ipcRendererInvoke = vi.fn()
const ipcRendererSend = vi.fn()
const ipcRendererOn = vi.fn((..._args: unknown[]) => vi.fn())
const ipcRendererOff = vi.fn()

vi.mock('../../../shared/tauri-renderer-runtime', () => ({
  ipcRenderer: {
    invoke: (...args: unknown[]) => ipcRendererInvoke(...args),
    send: (...args: unknown[]) => ipcRendererSend(...args),
    on: (...args: unknown[]) => ipcRendererOn(...args),
    off: (...args: unknown[]) => ipcRendererOff(...args),
    setMaxListeners: vi.fn(),
  },
}))

const createPluginRpcFailure = vi.fn()
const pluginRpcRequestSchemaSafeParse = vi.fn()
const pluginRpcResponseSchemaSafeParse = vi.fn()

vi.mock('../../../shared/plugin-rpc-contract', () => ({
  createPluginRpcFailure: (...args: unknown[]) => createPluginRpcFailure(...args),
  PluginRpcErrorCode: { InvalidRequest: 'invalid-request', HandlerFailed: 'handler-failed' },
  pluginRpcRequestSchema: {
    safeParse: (...args: unknown[]) => pluginRpcRequestSchemaSafeParse(...args),
  },
  pluginRpcResponseSchema: {
    safeParse: (...args: unknown[]) => pluginRpcResponseSchemaSafeParse(...args),
  },
}))

describe('shell api bridge (electronShellApi)', () => {
  let createElectronShellApi: typeof import('../electron-shell-api').createElectronShellApi

  beforeEach(async () => {
    vi.clearAllMocks()

    const mod = await import('../electron-shell-api')
    createElectronShellApi = mod.createElectronShellApi
  })

  it('creates an ElectronShellApi with platform and versions object', () => {
    const mika = { getHealth: vi.fn() }

    const api = createElectronShellApi(mika as never)

    expect(typeof api.platform).toBe('string')
    expect(api.versions).toHaveProperty('node')
    expect(api.versions).toHaveProperty('chrome')
    expect(api.versions).toHaveProperty('electron')
  })

  it('attaches runtime to the api', () => {
    const mika = { getHealth: vi.fn() }

    const api = createElectronShellApi(mika as never)

    expect(api.runtime).toBe(mika)
  })

  it('provides shell.openSettings', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    api.shell.openSettings?.()

    expect(ipcRendererInvoke).toHaveBeenCalledWith('open-settings')
  })

  it('provides shell.window methods', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    expect(api.shell.window?.close).toEqual(expect.any(Function))
    expect(api.shell.window?.toggleAlwaysOnTop).toEqual(expect.any(Function))
    expect(api.shell.window?.setIgnoreMouseEvents).toEqual(expect.any(Function))
    expect(api.shell.window?.moveBy).toEqual(expect.any(Function))
  })

  it('shell.window.close invokes the window-close channel', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    api.shell.window?.close()
    expect(ipcRendererInvoke).toHaveBeenCalledWith('window-close')
  })

  it('shell.window.toggleAlwaysOnTop invokes the channel and returns its result', async () => {
    ipcRendererInvoke.mockResolvedValue(true)
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    const result = await api.shell.window?.toggleAlwaysOnTop()
    expect(ipcRendererInvoke).toHaveBeenCalledWith('window-toggle-always-on-top')
    expect(result).toBe(true)
  })

  it('shell.window.toggleFullscreen / isFullscreen invoke their channels and return the window state', async () => {
    ipcRendererInvoke.mockResolvedValue(true)
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    expect(await api.shell.window?.toggleFullscreen?.()).toBe(true)
    expect(ipcRendererInvoke).toHaveBeenCalledWith('window-toggle-fullscreen')
    expect(await api.shell.window?.isFullscreen?.()).toBe(true)
    expect(ipcRendererInvoke).toHaveBeenCalledWith('window-get-fullscreen')
  })

  it('shell.window.setIgnoreMouseEvents sends (fire-and-forget) with the flag', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    api.shell.window?.setIgnoreMouseEvents(true)
    expect(ipcRendererSend).toHaveBeenCalledWith('window-ignore-mouse', true)
  })

  it('shell.window.moveBy sends window-move channel with deltas', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    api.shell.window?.moveBy?.(10, -5)
    expect(ipcRendererSend).toHaveBeenCalledWith('window-move', { deltaX: 10, deltaY: -5 })
  })

  it('shell.window.startDrag sends the native drag channel', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    api.shell.window?.startDrag?.()
    expect(ipcRendererSend).toHaveBeenCalledWith('window-start-drag')
  })

  it('shell.window.onMouseMove subscribes/unsubscribes and forwards the position', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    const onMove = vi.fn()
    const unsubscribe = api.shell.window?.onMouseMove(onMove)
    expect(ipcRendererOn).toHaveBeenCalledWith('mouse-move', expect.any(Function))
    const listener = ipcRendererOn.mock.calls.find((c) => c[0] === 'mouse-move')?.[1] as (
      e: unknown,
      pos: unknown,
    ) => void
    const pos = { x: 1, y: 2, insideX: 1, insideY: 2, isInside: true }
    listener(undefined, pos)
    expect(onMove).toHaveBeenCalledWith(pos)
    unsubscribe?.()
    expect(ipcRendererOff).toHaveBeenCalledWith('mouse-move', listener)
  })

  it('shell.autoLaunch.get and .set delegate to their channels', async () => {
    ipcRendererInvoke.mockResolvedValue(true)
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    const enabled = await api.shell.autoLaunch?.get()
    expect(ipcRendererInvoke).toHaveBeenCalledWith('auto-launch:get')
    expect(enabled).toBe(true)

    await api.shell.autoLaunch?.set(false)
    expect(ipcRendererInvoke).toHaveBeenCalledWith('auto-launch:set', false)
  })

  it('shell.system.preventSleep invokes with the requested flag', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    api.shell.system?.preventSleep(true)
    expect(ipcRendererInvoke).toHaveBeenCalledWith('system:preventSleep', { enabled: true })
  })

  it('provides shell.pluginViews.open', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    api.shell.pluginViews?.open({ pluginId: 'mika.test', viewId: 'main' })

    expect(ipcRendererInvoke).toHaveBeenCalledWith('plugin-view:open', {
      pluginId: 'mika.test',
      viewId: 'main',
    })
  })

  it('validates pluginRpc requests before sending', async () => {
    ipcRendererInvoke.mockResolvedValue({ ok: true })
    pluginRpcRequestSchemaSafeParse.mockReturnValue({
      success: false,
      error: { issues: [{ message: 'invalid' }] },
    })
    createPluginRpcFailure.mockReturnValue({
      success: false,
      error: { code: 'invalid-request', message: 'Invalid plugin RPC request' },
    })

    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    const result = await api.shell.pluginRpc?.({ pluginId: 'bad', method: '' })

    expect(result).toEqual({
      success: false,
      error: { code: 'invalid-request', message: 'Invalid plugin RPC request' },
    })
    expect(ipcRendererInvoke).toHaveBeenCalledWith(
      'mika:renderer-log',
      expect.objectContaining({ msg: 'invalid plugin RPC request' }),
    )
    expect(ipcRendererInvoke).not.toHaveBeenCalledWith('mika:plugin-rpc', expect.anything())
  })

  it('validates pluginRpc responses after sending', async () => {
    pluginRpcRequestSchemaSafeParse.mockReturnValue({
      success: true,
      data: { pluginId: 'mika.test', method: 'ping' },
    })
    ipcRendererInvoke.mockResolvedValue({ success: true, data: 'pong' })
    pluginRpcResponseSchemaSafeParse.mockReturnValue({
      success: true,
      data: { success: true, data: 'pong' },
    })

    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    const result = await api.shell.pluginRpc?.({ pluginId: 'mika.test', method: 'ping' })

    expect(result).toEqual({ success: true, data: 'pong' })
    expect(ipcRendererInvoke).toHaveBeenCalledWith('mika:plugin-rpc', {
      pluginId: 'mika.test',
      method: 'ping',
    })
  })

  it('handles invalid pluginRpc response with failure', async () => {
    pluginRpcRequestSchemaSafeParse.mockReturnValue({
      success: true,
      data: { pluginId: 'mika.test', method: 'ping' },
    })
    ipcRendererInvoke.mockResolvedValue({ success: true, data: 'pong' })
    pluginRpcResponseSchemaSafeParse.mockReturnValue({
      success: false,
      error: { issues: [{ message: 'invalid response' }] },
    })
    createPluginRpcFailure.mockReturnValue({
      success: false,
      error: { code: 'handler-failed', message: 'Invalid plugin RPC response' },
    })

    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    const result = await api.shell.pluginRpc?.({ pluginId: 'mika.test', method: 'ping' })

    expect(result).toEqual({
      success: false,
      error: { code: 'handler-failed', message: 'Invalid plugin RPC response' },
    })
  })

  it('provides shortcuts.onToggleInput and onToggleMic', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)

    const cleanup1 = api.shell.shortcuts?.onToggleInput(vi.fn())
    expect(ipcRendererOn).toHaveBeenCalledWith('shortcut:toggle-input', expect.any(Function))
    expect(cleanup1).toEqual(expect.any(Function))
    cleanup1?.()
    expect(ipcRendererOff).toHaveBeenCalled()

    const cleanup2 = api.shell.shortcuts?.onToggleMic(vi.fn())
    expect(ipcRendererOn).toHaveBeenCalledWith('shortcut:toggle-mic', expect.any(Function))
    cleanup2?.()
  })

  // The payload is the key edge; a truthy-but-not-boolean value must not hold the mic open.
  it('delivers the push-to-talk key edge as a boolean', () => {
    const api = createElectronShellApi({ getHealth: vi.fn() } as never)
    const callback = vi.fn()
    const cleanup = api.shell.shortcuts?.onPushToTalk(callback)
    const listener = ipcRendererOn.mock.calls.find(([channel]) => channel === 'shortcut:push-to-talk')?.[1] as
      | ((event: unknown, payload: unknown) => void)
      | undefined
    listener?.({}, { pressed: true })
    listener?.({}, { pressed: 'yes' })
    listener?.({}, { pressed: false })
    expect(callback.mock.calls).toEqual([[true], [false], [false]])
    cleanup?.()
  })
})
