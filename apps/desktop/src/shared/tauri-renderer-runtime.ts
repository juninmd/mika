/**
 * `ipcRenderer` e `contextBridge` sobre Tauri.
 *
 * Existe para que os 474 linhas de `src/renderer/bridge/` — contrato congelado em preload.module.ts —
 * continuem valendo sem uma linha alterada. O preload importa esses dois nomes de
 * `electron-runtime.ts`; o alias do bundler aponta para cá no alvo Tauri.
 *
 * Diferença de fundo: no Electron isso rodava num processo isolado com contextIsolation.
 * Em Tauri não há preload — o webview só alcança o que os comandos `mika_invoke`/`mika_send`
 * expõem, e o que cada janela pode chamar é decidido pelas capabilities, não por este arquivo.
 */

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/**
 * `any`, não `unknown`: espelha `(event: IpcRendererEvent, ...args: any[]) => void` do
 * `ipcRenderer` real do Electron — trocar por `unknown` quebra os ~474 linhas de chamadores
 * tipados em `src/renderer/bridge/` que esperavam o `any` permissivo.
 */
// biome-ignore lint/suspicious/noExplicitAny: ver comentário acima
type Listener = (event: any, payload: any) => void

/**
 * `ipcRenderer.on` é síncrono e devolve imediatamente; `listen` do Tauri é assíncrono.
 * Guardar a promessa do unlisten aqui mantém a assinatura síncrona que o preload espera —
 * sem isso, um `off()` chamado antes do listen resolver vazaria o listener.
 */
const unlisteners = new Map<string, Promise<UnlistenFn>>()

/** Keyed by channel AND listener: the same handler may serve several channels. */
const unlistenKey = (channel: string, listener: Listener): string => `${channel}\u0000${listenerId(listener)}`
const listenerIds = new WeakMap<Listener, number>()
let nextListenerId = 0
function listenerId(listener: Listener): number {
  let id = listenerIds.get(listener)
  if (id === undefined) {
    id = nextListenerId++
    listenerIds.set(listener, id)
  }
  return id
}

function release(key: string): void {
  const pending = unlisteners.get(key)
  if (!pending) return
  unlisteners.delete(key)
  void pending.then((unlisten) => unlisten()).catch(() => {})
}

/**
 * `payload` fica FORA do objeto quando a rota não recebe argumento — nunca `payload: null`.
 *
 * JSON não tem `undefined`. O `?? null` que estava aqui fazia toda rota sem argumento chegar ao
 * Zod do `router.ts` como `null` e ser recusada com "expected undefined, received null": o app
 * abria com tema, i18n, config, histórico e health rejeitados pelo próprio contrato. Omitindo a
 * chave, o lado Rust não a serializa (`skip_serializing_if` em `protocol.rs`) e o sidecar lê
 * `undefined`, igual ao Electron — enquanto um `null` passado de propósito continua chegando
 * como `null`.
 */
/**
 * TypedArray vira array comum antes de sair — o Electron mandava por structured clone, que
 * PRESERVA `Float32Array`; aqui o caminho é JSON, e `JSON.stringify(new Float32Array([1,2]))`
 * devolve `{"0":1,"1":2}`, um objeto de chaves numéricas.
 *
 * Custou uma feature inteira: `voice:vad-audio` declara
 * `samples: z.union([z.instanceof(Float32Array), z.array(z.number())])` e nenhum dos dois ramos
 * casa com esse objeto, então TODO quadro de áudio do VAD era recusado pelo contrato e o STT
 * nunca recebia nada. No log aparecia só `request rejected {"channel":"voice:vad-audio"}`, sem
 * dizer que o formato tinha mudado no caminho.
 *
 * A conversão é aqui, e não em cada chamador, porque o chokepoint é que existe para fazer o
 * alvo Tauri se comportar como o Electron. `JSON.stringify` já seria chamado logo depois, então
 * isto não introduz nenhuma falha nova — só preserva a forma que o outro lado espera.
 */
function toJsonSafe(value: unknown): unknown {
  if (ArrayBuffer.isView(value)) return Array.from(value as unknown as ArrayLike<number>)
  if (Array.isArray(value)) return value.map(toJsonSafe)
  if (value === null || typeof value !== 'object') return value
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, toJsonSafe(item)]))
}

/**
 * `payload` fica FORA do objeto quando a rota não recebe argumento — nunca `payload: null`.
 *
 * JSON não tem `undefined`. O `?? null` que estava aqui fazia toda rota sem argumento chegar ao
 * Zod do `router.ts` como `null` e ser recusada com "expected undefined, received null": o app
 * abria com tema, i18n, config, histórico e health rejeitados pelo próprio contrato. Omitindo a
 * chave, o lado Rust não a serializa (`skip_serializing_if` em `protocol.rs`) e o sidecar lê
 * `undefined`, igual ao Electron — enquanto um `null` passado de propósito continua chegando
 * como `null`.
 */
function args(channel: string, payload?: unknown): Record<string, unknown> {
  return payload === undefined ? { channel } : { channel, payload: toJsonSafe(payload) }
}

const VAD_AUDIO_CHANNEL = 'voice:vad-audio'

/**
 * Quadro do VAD (até ~100 Hz) como corpo binário: Float32 little-endian cru, `amplitude`/`playing`
 * nos headers. O caminho JSON (`Array.from` + `JSON.stringify` + parse no Rust) alocava dezenas de
 * milhares de números por segundo. Só `Float32Array` entra aqui; `number[]` segue pelo `mika_send`
 * JSON (o Rust aceita os dois, ver `vad_audio.rs::decode_f32_le`).
 */
function sendVadAudioBinary(payload: { samples: Float32Array; amplitude: number; playing?: boolean }): void {
  const headers: Record<string, string> = { 'x-mika-amplitude': String(payload.amplitude) }
  if (payload.playing !== undefined) headers['x-mika-playing'] = payload.playing ? '1' : '0'
  const bytes = new Uint8Array(payload.samples.buffer, payload.samples.byteOffset, payload.samples.byteLength)
  void invoke('mika_send_pcm', bytes, { headers }).catch(() => {})
}

function isBinaryVadPayload(
  channel: string,
  payload: unknown,
): payload is { samples: Float32Array; amplitude: number; playing?: boolean } {
  if (channel !== VAD_AUDIO_CHANNEL || typeof payload !== 'object' || payload === null) return false
  const { samples, amplitude } = payload as { samples?: unknown; amplitude?: unknown }
  return samples instanceof Float32Array && typeof amplitude === 'number'
}

export const ipcRenderer = {
  // biome-ignore lint/suspicious/noExplicitAny: mesma permissividade de Promise<any> do ipcRenderer.invoke real do Electron — cada chamador em src/renderer/bridge/ já tipa o retorno.
  invoke(channel: string, payload?: unknown): Promise<any> {
    return invoke('mika_invoke', args(channel, payload))
  },

  send(channel: string, payload?: unknown): void {
    if (isBinaryVadPayload(channel, payload)) {
      sendVadAudioBinary(payload)
      return
    }
    void invoke('mika_send', args(channel, payload)).catch(() => {
      // `send` é disparo sem resposta; o Electron também não propagava erro daqui.
    })
  },

  on(channel: string, listener: Listener): void {
    const key = unlistenKey(channel, listener)
    release(key)
    const pending = listen(channel, (event) => listener({}, event.payload))
    // A rejected `listen` (event plugin not ready) must not become an unhandled rejection; `off` swallows it too.
    pending.catch(() => {})
    unlisteners.set(key, pending)
  },

  /**
   * No-op: o teto de listeners é do EventEmitter do Node, que o `ipcRenderer` do Electron
   * estendia. O `listen` do Tauri não tem limite nem emite aviso, então não há o que ajustar.
   * Existe porque o preload chama — sem isto o boot lança e a janela sobe em branco.
   */
  setMaxListeners(_limit: number): void {},

  off(channel: string, listener: Listener): void {
    release(unlistenKey(channel, listener))
  },
}

export const contextBridge = {
  /**
   * Sem processo isolado, expor é atribuir. Congelado para que a superfície não possa ser
   * remendada em runtime por script carregado depois.
   */
  exposeInMainWorld(key: string, api: unknown): void {
    Object.defineProperty(globalThis, key, {
      value: Object.freeze(api),
      writable: false,
      configurable: false,
      enumerable: true,
    })
  },
}
