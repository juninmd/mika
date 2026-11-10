/**
 * Regressão de 2026-09-21, achada executando o app empacotado.
 *
 * `ipcRenderer` mandava `payload: payload ?? null`. JSON não tem `undefined`, então toda rota sem
 * argumento chegava ao Zod do `router.ts` como `null` e era recusada pelo próprio contrato —
 * `Invalid IPC request for mika:get-health — root: Invalid input: expected undefined, received
 * null`. O app abria com tema, i18n, config, histórico de chat e health todos rejeitados.
 *
 * A chave tem de ficar FORA do objeto: o lado Rust não a serializa
 * (`skip_serializing_if = "Option::is_none"` em `protocol.rs`) e o sidecar lê `undefined`, igual
 * ao Electron. Um `null` passado de propósito continua tendo de chegar como `null` — é essa
 * distinção que o `?? null` apagava.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.fn(() => Promise.resolve(null))
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...(a as [])) }))
const listen = vi.fn((_channel: string, _handler: unknown) => Promise.resolve(() => {}))
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a: unknown[]) => listen(...(a as [string, unknown])) }))

const { ipcRenderer } = await import('../tauri-renderer-runtime')

beforeEach(() => {
  invoke.mockClear()
  listen.mockClear()
})

describe('ipcRenderer não inventa payload', () => {
  it('rota sem argumento não leva a chave `payload`', async () => {
    await ipcRenderer.invoke('mika:get-health')

    const [command, args] = invoke.mock.calls[0] as unknown as [string, Record<string, unknown>]
    expect(command).toBe('mika_invoke')
    expect(args).toEqual({ channel: 'mika:get-health' })
    expect('payload' in args).toBe(false)
  })

  it('um null enviado de propósito continua sendo null, não ausência', async () => {
    await ipcRenderer.invoke('mika:algum-canal', null)

    const [, args] = invoke.mock.calls[0] as unknown as [string, Record<string, unknown>]
    expect(args).toEqual({ channel: 'mika:algum-canal', payload: null })
  })

  it('payload normal atravessa intacto', async () => {
    await ipcRenderer.invoke('mika:settings-set', { key: 'voice.enabled', value: true })

    const [, args] = invoke.mock.calls[0] as unknown as [string, Record<string, unknown>]
    expect(args).toEqual({ channel: 'mika:settings-set', payload: { key: 'voice.enabled', value: true } })
  })

  it('`send` segue a mesma regra — era o outro caminho com o mesmo `?? null`', () => {
    ipcRenderer.send('mika:chat-clear')

    const [command, args] = invoke.mock.calls[0] as unknown as [string, Record<string, unknown>]
    expect(command).toBe('mika_send')
    expect('payload' in args).toBe(false)
  })

  /*
   * O Electron mandava por structured clone, que preserva `Float32Array`. Em JSON,
   * `JSON.stringify(new Float32Array([1,2]))` vira `{"0":1,"1":2}`, que o schema de `voice:vad-audio`
   * recusa — e antes de `mika_send_pcm` o remédio era `Array.from` em ~100 quadros/s. Agora o quadro
   * Float32 vai como corpo binário (Float32 LE) e a amplitude/playing nos headers; o JSON fica para
   * quem manda `number[]`.
   */
  it('VAD com Float32Array vai binário (mika_send_pcm), sem JSON nem Array.from', () => {
    const samples = new Float32Array([0.5, -0.25])
    ipcRenderer.send('voice:vad-audio', { samples, amplitude: 0.3, playing: true })

    const [command, body, options] = invoke.mock.calls[0] as unknown as [
      string,
      Uint8Array,
      { headers: Record<string, string> },
    ]
    expect(command).toBe('mika_send_pcm')
    expect(body).toBeInstanceOf(Uint8Array)
    expect(Array.from(new Float32Array(body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength)))).toEqual(
      [0.5, -0.25],
    )
    expect(options.headers).toEqual({ 'x-mika-amplitude': '0.3', 'x-mika-playing': '1' })
  })

  it('VAD sem `playing` não manda o header — o Rust distingue ausente de falso', () => {
    ipcRenderer.send('voice:vad-audio', { samples: new Float32Array([0.1]), amplitude: 0 })

    const [, , options] = invoke.mock.calls[0] as unknown as [string, Uint8Array, { headers: Record<string, string> }]
    expect(options.headers).toEqual({ 'x-mika-amplitude': '0' })
  })

  it('VAD com number[] continua pelo caminho JSON compatível', () => {
    ipcRenderer.send('voice:vad-audio', { samples: [0.5, -0.25], amplitude: 0.3 })

    const [command, args] = invoke.mock.calls[0] as unknown as [
      string,
      { channel: string; payload: { samples: number[] } },
    ]
    expect(command).toBe('mika_send')
    expect(args.channel).toBe('voice:vad-audio')
    expect(args.payload.samples).toEqual([0.5, -0.25])
  })

  it('a conversão alcança TypedArray aninhado, não só o topo do payload', async () => {
    await ipcRenderer.invoke('mika:algum-canal', { a: { b: [new Uint8Array([1, 2])] }, c: 'texto' })

    const [, args] = invoke.mock.calls[0] as unknown as [string, { payload: unknown }]
    expect(args.payload).toEqual({ a: { b: [[1, 2]] }, c: 'texto' })
  })
})

describe('ipcRenderer.on / off', () => {
  // Regression: unlisteners were keyed by listener alone, so one handler on two channels kept only the
  // last unlisten and `off()` on either channel leaked the other subscription.
  it('the same listener on two channels is unsubscribed per channel', async () => {
    const unlistenA = vi.fn()
    const unlistenB = vi.fn()
    listen.mockReturnValueOnce(Promise.resolve(unlistenA)).mockReturnValueOnce(Promise.resolve(unlistenB))
    const handler = vi.fn()

    ipcRenderer.on('chan:a', handler)
    ipcRenderer.on('chan:b', handler)
    ipcRenderer.off('chan:a', handler)
    await Promise.resolve()
    await Promise.resolve()

    expect(unlistenA).toHaveBeenCalledTimes(1)
    expect(unlistenB).not.toHaveBeenCalled()

    ipcRenderer.off('chan:b', handler)
    await Promise.resolve()
    await Promise.resolve()
    expect(unlistenB).toHaveBeenCalledTimes(1)
  })

  it('a rejected listen() does not surface as an unhandled rejection', async () => {
    const unhandled = vi.fn()
    process.on('unhandledRejection', unhandled)
    listen.mockReturnValueOnce(Promise.reject(new Error('event plugin not ready')))

    ipcRenderer.on('chan:broken', vi.fn())
    await new Promise((resolve) => setTimeout(resolve, 10))
    process.off('unhandledRejection', unhandled)

    expect(unhandled).not.toHaveBeenCalled()
  })

  it('re-registering the same channel+listener replaces (and releases) the previous subscription', async () => {
    const first = vi.fn()
    listen.mockReturnValueOnce(Promise.resolve(first))
    const handler = vi.fn()

    ipcRenderer.on('chan:dup', handler)
    ipcRenderer.on('chan:dup', handler)
    await Promise.resolve()
    await Promise.resolve()

    expect(first).toHaveBeenCalledTimes(1)
    ipcRenderer.off('chan:dup', handler)
  })
})
