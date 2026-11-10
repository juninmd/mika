import { DESKTOP_INVOKE_CHANNELS, DESKTOP_SEND_CHANNELS } from '@mika/sdk/contracts'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ipcRendererInvoke, ipcRendererOff, ipcRendererOn, ipcRendererSend, setMaxListeners } from './ipc-fakes'

vi.mock('../../../shared/tauri-renderer-runtime', () => ({
  ipcRenderer: {
    invoke: (...args: unknown[]) => ipcRendererInvoke(...args),
    send: (...args: unknown[]) => ipcRendererSend(...args),
    on: (...args: unknown[]) => ipcRendererOn(...args),
    off: (...args: unknown[]) => ipcRendererOff(...args),
    setMaxListeners: (...args: unknown[]) => setMaxListeners(...args),
  },
}))

describe('mika-runtime-api', () => {
  let mod: typeof import('../mika-runtime-api')

  beforeEach(async () => {
    vi.clearAllMocks()
    vi.resetModules()
    mod = await import('../mika-runtime-api')
  })

  it('caps the ipcRenderer listener limit on module load', () => {
    expect(setMaxListeners).toHaveBeenCalledWith(20)
  })

  it('sendRendererLog resolves true when the main process acknowledges', async () => {
    ipcRendererInvoke.mockResolvedValueOnce({ ok: true })
    const result = await mod.sendRendererLog({ level: 'info', message: 'hi' } as never)
    expect(ipcRendererInvoke).toHaveBeenCalledWith(DESKTOP_INVOKE_CHANNELS.RendererLog, {
      level: 'info',
      message: 'hi',
    })
    expect(result).toBe(true)
  })

  it('sendRendererLog resolves false when invoke rejects instead of throwing', async () => {
    ipcRendererInvoke.mockRejectedValueOnce(new Error('bridge down'))
    const result = await mod.sendRendererLog({ level: 'error', message: 'x' } as never)
    expect(result).toBe(false)
  })

  it('sendRendererLog resolves false when the response has no ok flag', async () => {
    ipcRendererInvoke.mockResolvedValueOnce({})
    const result = await mod.sendRendererLog({ level: 'info', message: 'x' } as never)
    expect(result).toBe(false)
  })

  it('createMikaRuntimeApi.getHealth invokes the health channel with no payload', () => {
    const api = mod.createMikaRuntimeApi()
    api.getHealth()
    expect(ipcRendererInvoke).toHaveBeenCalledWith(DESKTOP_INVOKE_CHANNELS.GetHealth)
  })

  it('sendChatMessage forwards message, attachments and source as one payload', () => {
    const api = mod.createMikaRuntimeApi()
    api.sendChatMessage('oi', [{ type: 'image', mimeType: 'image/png', data: 'x' }], 'voice')
    expect(ipcRendererInvoke).toHaveBeenCalledWith(DESKTOP_INVOKE_CHANNELS.ChatSend, {
      message: 'oi',
      attachments: [{ type: 'image', mimeType: 'image/png', data: 'x' }],
      source: 'voice',
    })
  })

  it('sendVadAudio uses send, not invoke, since it is fire-and-forget', () => {
    const api = mod.createMikaRuntimeApi()
    api.sendVadAudio({ samples: [1, 2], amplitude: 0.5 })
    expect(ipcRendererSend).toHaveBeenCalledWith(DESKTOP_SEND_CHANNELS.VoiceVadAudio, {
      samples: [1, 2],
      amplitude: 0.5,
    })
    expect(ipcRendererInvoke).not.toHaveBeenCalled()
  })

  it('saveSetting forwards path and value as one payload', () => {
    const api = mod.createMikaRuntimeApi()
    api.saveSetting('voice.enabled', true)
    expect(ipcRendererInvoke).toHaveBeenCalledWith(DESKTOP_INVOKE_CHANNELS.SettingsSet, {
      path: 'voice.enabled',
      value: true,
    })
  })

  it('executeTool forwards name, args and origin as one payload', () => {
    const api = mod.createMikaRuntimeApi()
    api.executeTool('search', { q: '1' }, 'turn')
    expect(ipcRendererInvoke).toHaveBeenCalledWith(DESKTOP_INVOKE_CHANNELS.ToolExecute, {
      name: 'search',
      args: { q: '1' },
      origin: 'turn',
    })
  })

  it('onTTSAudio subscribes to the channel and unsubscribes via the returned function', () => {
    const off = vi.fn()
    ipcRendererOn.mockImplementationOnce(() => off)
    const api = mod.createMikaRuntimeApi()
    const callback = vi.fn()
    const unsubscribe = api.onTTSAudio(callback)
    const [, listener] = ipcRendererOn.mock.calls[0] as [string, (event: unknown, payload: unknown) => void]
    listener(undefined, { audio: 'base64', mimeType: 'audio/mpeg' })
    expect(callback).toHaveBeenCalledWith({ audio: 'base64', mimeType: 'audio/mpeg' })

    unsubscribe()
    expect(ipcRendererOff).toHaveBeenCalledWith(expect.any(String), listener)
  })

  it('onListeningCommand merges started/stopped channels into one boolean callback and unsubscribes both', () => {
    const api = mod.createMikaRuntimeApi()
    const callback = vi.fn()
    const unsubscribe = api.onListeningCommand(callback)
    expect(ipcRendererOn).toHaveBeenCalledTimes(2)

    const [, startedListener] = ipcRendererOn.mock.calls[0] as [string, (event: unknown, payload: unknown) => void]
    const [, stoppedListener] = ipcRendererOn.mock.calls[1] as [string, (event: unknown, payload: unknown) => void]
    startedListener(undefined, undefined)
    expect(callback).toHaveBeenCalledWith({ listening: true })
    stoppedListener(undefined, undefined)
    expect(callback).toHaveBeenCalledWith({ listening: false })

    unsubscribe()
    expect(ipcRendererOff).toHaveBeenCalledTimes(2)
  })

  it('onMicState requests the current state on subscribe and forwards updates', () => {
    const api = mod.createMikaRuntimeApi()
    const callback = vi.fn()
    api.onMicState(callback)
    expect(ipcRendererSend).toHaveBeenCalledWith(DESKTOP_SEND_CHANNELS.VoiceMicStateRequest)

    const [, listener] = ipcRendererOn.mock.calls[0] as [string, (event: unknown, payload: unknown) => void]
    listener(undefined, { enabled: true })
    expect(callback).toHaveBeenCalledWith({ enabled: true })
  })

  it('onLog fans a batch payload out into one callback call per log item', () => {
    const api = mod.createMikaRuntimeApi()
    const callback = vi.fn()
    api.onLog(callback)
    const [, listener] = ipcRendererOn.mock.calls[0] as [string, (event: unknown, payload: unknown) => void]
    listener(undefined, [{ id: '1' }, { id: '2' }])
    expect(callback).toHaveBeenCalledTimes(2)
    expect(callback).toHaveBeenNthCalledWith(1, { id: '1' })
    expect(callback).toHaveBeenNthCalledWith(2, { id: '2' })
  })

  it('sendToolApprovalResponse forwards toolCallId, approved and scope', () => {
    const api = mod.createMikaRuntimeApi()
    api.sendToolApprovalResponse('call-1', true, 'session')
    expect(ipcRendererSend).toHaveBeenCalledWith(DESKTOP_SEND_CHANNELS.ToolApprovalResponse, {
      toolCallId: 'call-1',
      approved: true,
      scope: 'session',
    })
  })

  it('resetProviderCircuit forwards provider and optional model', () => {
    const api = mod.createMikaRuntimeApi()
    api.resetProviderCircuit('openrouter', 'gpt')
    expect(ipcRendererInvoke).toHaveBeenCalledWith(expect.any(String), { provider: 'openrouter', model: 'gpt' })
  })
})
