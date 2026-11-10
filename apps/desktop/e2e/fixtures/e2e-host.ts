/**
 * Cliente Playwright do host HTTP do E2E (`src-tauri/src/e2e_host.rs`) — o caminho de Linux e macOS.
 *
 * CDP só existe no WebView2; WebKitGTK/WKWebView não o expõem. Aqui o Playwright abre um Chromium
 * nas páginas do app, servidas pelo próprio binário, e um shim de `window.__TAURI_INTERNALS__`
 * leva `invoke`/`listen` — as ÚNICAS duas funções do Tauri que o renderer usa
 * (`src/shared/tauri-renderer-runtime.ts`) — até o backend Rust real por loopback. O contrato com
 * as specs é o mesmo do caminho CDP: uma `Page` real do Playwright por janela.
 */
import type { ChildProcess } from 'node:child_process'
import { chromium, type Browser, type BrowserContext, type Page } from '@playwright/test'

type HostWindow = { label: string; path: string | null; width: number | null; height: number | null }

/** Fora do Windows não há CDP: o host HTTP é o único caminho. `MIKA_E2E_HOST=1` o força no Windows. */
export const isHostMode = process.platform !== 'win32' || process.env.MIKA_E2E_HOST === '1'

/** Roda no Chromium ANTES do app: monta a ponte Tauri e os mesmos shims que `windows.rs` injeta. */
function shimSource(label: string, token: string, platform: string): string {
  return `(() => {
  const label = ${JSON.stringify(label)}, token = ${JSON.stringify(token)}
  const headers = { 'content-type': 'application/json', 'x-mika-e2e-token': token }
  const post = (path, body, extra = {}) => fetch('/__e2e/' + path, { method: 'POST', headers: { ...headers, ...extra }, body }).then((r) => r.json())
  const streamId = crypto.randomUUID()
  const callbacks = new Map()
  const handlers = new Map()
  let nextCallback = 1
  const source = new EventSource('/__e2e/stream?id=' + streamId + '&token=' + token)
  const ready = new Promise((resolve) => { source.onopen = resolve })
  source.onmessage = (message) => {
    const { event, handler, eventId, payload } = JSON.parse(message.data)
    callbacks.get(handler)?.({ event, id: eventId, payload })
  }
  // O app assina eventos de forma assíncrona depois de montar; um evento emitido antes da assinatura
  // chegar ao backend se perde. A harness espera esta fila esvaziar antes de entregar a página.
  const listenState = { inflight: 0, last: Date.now() }
  window.__MIKA_E2E_LISTEN_STATE__ = () => ({ inflight: listenState.inflight, quietMs: Date.now() - listenState.last })
  const unwrap = (reply) => { if ('err' in reply) throw reply.err; return reply.ok }
  const unsupported = (cmd) => { throw new Error('comando Tauri sem suporte no host E2E: ' + cmd) }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label }, currentWebview: { windowLabel: label, label } },
    transformCallback(callback, once) {
      const id = nextCallback++
      callbacks.set(id, (value) => { if (once) callbacks.delete(id); callback(value) })
      return id
    },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (path) => path,
    async invoke(cmd, args = {}, options) {
      if (cmd === 'mika_invoke' || cmd === 'mika_send') {
        return unwrap(await post('invoke', JSON.stringify({ label, channel: args.channel, payload: args.payload })))
      }
      if (cmd === 'mika_send_pcm') {
        return unwrap(await post('pcm', args, { 'content-type': 'application/octet-stream', 'x-mika-label': label, ...(options?.headers ?? {}) }))
      }
      if (cmd === 'plugin:event|listen') {
        listenState.inflight++
        try {
          await ready
          const eventId = unwrap(await post('listen', JSON.stringify({ stream: streamId, label, event: args.event, handler: args.handler })))
          handlers.set(eventId, args.handler)
          return eventId
        } finally {
          listenState.inflight--
          listenState.last = Date.now()
        }
      }
      if (cmd === 'plugin:event|unlisten') {
        callbacks.delete(handlers.get(args.eventId))
        handlers.delete(args.eventId)
        return unwrap(await post('unlisten', JSON.stringify({ stream: streamId, eventId: args.eventId })))
      }
      return unsupported(cmd)
    },
  }
  // O Chromium não conhece o esquema \`mika-live2d:\`: o host serve os mesmos arquivos em /__e2e/live2d/.
  const L2D = 'mika-live2d://localhost/'
  const live2d = (url) => (typeof url === 'string' && url.startsWith(L2D) ? '/__e2e/live2d/' + url.slice(L2D.length) + (url.includes('?') ? '&' : '?') + 'token=' + token : url)
  const realFetch = window.fetch.bind(window)
  window.fetch = (input, init) => realFetch(typeof input === 'string' ? live2d(input) : input, init)
  const realOpen = XMLHttpRequest.prototype.open
  XMLHttpRequest.prototype.open = function (method, url, ...rest) { return realOpen.call(this, method, live2d(String(url)), ...rest) }
  const srcSetter = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src')
  Object.defineProperty(HTMLImageElement.prototype, 'src', { ...srcSetter, set(value) { srcSetter.set.call(this, live2d(value)) } })
  window.close = () => window.__TAURI_INTERNALS__.invoke('mika_invoke', { channel: 'window-close' })
  globalThis.process = { platform: ${JSON.stringify(platform)}, env: {}, versions: { chrome: (navigator.userAgent.match(/Chrome\\/([\\d.]+)/) || [])[1] || '' } }
})()`
}

export type HostSession = {
  context: BrowserContext
  /** Primeira janela do app (a `main`, ou a que o boot abriu primeiro). */
  firstPage: () => Page
  /** Resolve na próxima janela que o backend abrir. */
  nextWindow: (timeoutMs: number) => Promise<Page>
  close: () => Promise<void>
}

const POLL_MS = 120

/** Espera o app terminar de assinar eventos (nenhuma assinatura em voo e 400 ms sem novas). */
async function settleListeners(page: Page): Promise<void> {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    const state = await page
      .evaluate(() => (window as unknown as { __MIKA_E2E_LISTEN_STATE__?: () => { inflight: number; quietMs: number } }).__MIKA_E2E_LISTEN_STATE__?.())
      .catch(() => undefined)
    if (state && state.inflight === 0 && state.quietMs > 400) return
    await new Promise((r) => setTimeout(r, 100))
  }
}

export async function connectHost(
  port: number,
  token: string,
  child: ChildProcess,
  startupMs: number,
): Promise<HostSession> {
  const origin = `http://127.0.0.1:${port}`
  const api = async <T>(path: string): Promise<T> => {
    const response = await fetch(`${origin}/__e2e/${path}`, { headers: { 'x-mika-e2e-token': token } })
    if (!response.ok) throw new Error(`${path} -> ${response.status}`)
    return (await response.json()) as T
  }

  const browser: Browser = await chromium.launch({
    // Chromium já instalado na máquina (ex.: imagem sem acesso ao CDN do Playwright).
    executablePath: process.env.MIKA_E2E_CHROMIUM || undefined,
    args: [
      '--use-fake-ui-for-media-stream',
      '--use-fake-device-for-media-stream',
      '--autoplay-policy=no-user-gesture-required',
      '--use-angle=swiftshader',
      '--enable-unsafe-swiftshader',
    ],
  })
  const context = await browser.newContext({ permissions: ['microphone'], baseURL: origin })
  const pages = new Map<string, Page>()
  const order: Page[] = []
  const waiting: Array<(page: Page) => void> = []
  let stopped = false

  const open = async (win: HostWindow) => {
    const page = await context.newPage()
    const rawClose = page.close.bind(page)
    // `window.close()` pelo caminho do usuário: o backend fecha a janela e o poll fecha a página.
    page.close = async () => {
      if (page.isClosed()) return
      const closed = page.waitForEvent('close')
      if (win.label.startsWith('plugin-view')) {
        await fetch(`${origin}/__e2e/close`, { method: 'POST', headers: { 'x-mika-e2e-token': token }, body: JSON.stringify({ label: win.label }) })
      } else await page.evaluate(() => window.close()).catch(() => undefined)
      await closed
    }
    ;(page as Page & { rawClose?: () => Promise<void> }).rawClose = rawClose
    await page.addInitScript(shimSource(win.label, token, process.platform === 'darwin' ? 'darwin' : 'linux'))
    if (win.width && win.height) await page.setViewportSize({ width: Math.round(win.width), height: Math.round(win.height) })
    await page.goto(`${origin}/${win.path ?? 'index.html'}`)
    await settleListeners(page)
    // Só agora a página existe para quem pergunta: antes do `goto` ela é um `about:blank` cujo
    // contexto de execução morre na navegação, e um `evaluate` logo após `firstPage()` explodia.
    pages.set(win.label, page)
    order.push(page)
    for (const resolve of waiting.splice(0)) resolve(page)
  }

  const sync = async () => {
    const windows = await api<HostWindow[]>('windows')
    const live = new Set(windows.map((w) => w.label))
    for (const win of windows) if (win.path && !pages.has(win.label)) await open(win)
    for (const [label, page] of pages) {
      if (live.has(label)) continue
      pages.delete(label)
      await (page as Page & { rawClose?: () => Promise<void> }).rawClose?.().catch(() => undefined)
    }
  }

  const loop = (async () => {
    while (!stopped) {
      await sync().catch(() => undefined)
      await new Promise((r) => setTimeout(r, POLL_MS))
    }
  })()

  const deadline = Date.now() + startupMs
  while (order.length === 0) {
    if (child.exitCode !== null) throw new Error(`o app saiu (código ${child.exitCode}) antes de abrir uma janela`)
    if (Date.now() > deadline) throw new Error(`nenhuma janela do app em ${startupMs}ms (host ${origin})`)
    await new Promise((r) => setTimeout(r, POLL_MS))
  }

  return {
    context,
    firstPage: () => order[0] as Page,
    nextWindow: (timeoutMs) =>
      new Promise<Page>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`nenhuma janela nova em ${timeoutMs}ms`)), timeoutMs)
        waiting.push((page) => {
          clearTimeout(timer)
          resolve(page)
        })
      }),
    close: async () => {
      stopped = true
      await loop.catch(() => undefined)
      await browser.close().catch(() => undefined)
    },
  }
}
