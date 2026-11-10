/**
 * Sucessor de `electron-app.ts` (removido) — dirige o binário TAURI via CDP (WebView2), não
 * `_electron.launch()`. Nasceu em `../tauri-cdp/fixtures/tauri-cdp-app.ts` (provado 3/3: boot,
 * chat-send, settings) e foi promovido a fixture canônica quando o Electron saiu do app.
 *
 * Por que CDP em vez de WebdriverIO (`../tauri/`): o WDIO/`@wdio/tauri-service` (driver
 * `external`, único viável sem editar `src-tauri/`) sobe o app FORA de qualquer hook por-teste —
 * `beforeSession` roda uma vez por worker, não por `it()` (ver `../tauri/README.md`, "O que se
 * perde" #2) — e reescreveria a asserção (`expect-webdriverio`/mocha) das specs existentes. Este
 * fixture, ao contrário, expõe `app.firstWindow()` retornando uma `Page` REAL do
 * `@playwright/test` conectada por `chromium.connectOverCDP()`: as specs que só usam
 * `page.locator/evaluate/waitFor*` trocam apenas a linha de import
 * (`'../fixtures/electron-app'` → `'../fixtures/tauri-app'`) e continuam de pé, com `expect` do
 * Playwright, sem tocar a asserção.
 *
 * `app.evaluate()` (Electron main process — `BrowserWindow`, `app.exit`, `powerMonitor`) NÃO tem
 * equivalente aqui — ver o `throw` abaixo com a lista de specs que ainda dependem disso.
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { join, resolve } from 'node:path'
import { chromium, test as base, type Browser, type BrowserContext, type Page, expect } from '@playwright/test'
import { AVATAR_MODELS } from '../../src/renderer/src/components/live2d/model-path'
import { startFakeOllama, type FakeOllama } from './fake-ollama-server'
import { appBinaryPath } from './app-binary'
import { connectHost, type HostSession, isHostMode } from './e2e-host'
import { legalAccepted } from './legal-accepted'
import { startFakeWiki, type FakeWiki } from './fake-wiki-server'
import { createTempProfile, type TempProfile } from './temp-profile'

// Playwright compila specs/fixtures de `e2e/**` em CJS — `import.meta.url` quebraria com "Cannot
// use 'import.meta' outside a module"; `__dirname` é o global certo aqui.
// Re-exported so other Tauri-CDP fixtures (e.g. `../homologation/fixtures/homolog-app.ts`) can spawn
// the same binary; the path itself is resolved once, in `./app-binary`.
export { appBinaryPath }

/**
 * Casca fina em cima da `Page` real para as specs que só chamam `app.firstWindow()` uma vez no
 * topo do teste — o resto do arquivo (`page.locator(...)`, `page.evaluate(...)`) não muda.
 */
export type TauriShellApp = {
  firstWindow: () => Promise<Page>
  // Electron's `app.waitForEvent('window')` resolves when a second `BrowserWindow` opens (e.g.
  // clicking "Configurações"). Tauri mirrors that with a second `WebviewWindow` (`windows.rs`),
  // which surfaces here as a new `page` on the same CDP-connected `BrowserContext`.
  // `'close'` resolves when the spawned process actually exits — needed by specs that verify
  // graceful shutdown (`quit` via the e2e hook) persisted state before the process died.
  waitForEvent: {
    (event: 'window'): Promise<Page>
    (event: 'close'): Promise<void>
  }
  evaluate: (..._args: unknown[]) => never
  // Full child process, not just the pid: `stdout`/`stderr` are the real Node streams (already
  // piped to this process' own stdout/stderr above; a spec can attach its own `'data'` listener
  // on top without stealing the pipe — Node streams fan out to every listener).
  process: () => ChildProcess
  close: () => Promise<void>
}

type MikaFixtures = {
  app: TauriShellApp
  profile: TempProfile
  llm: FakeOllama
  wiki: FakeWiki
  cdpPort: number
}

// Skin pinned via `test.use({ skin: 'haru' })`, not process.env — workers:1 shares a process, so an env var would leak across specs.
type MikaOptions = {
  skin: string | undefined
}

/** Instância órfã do plugin single-instance faz a próxima sessão morrer calada no handshake. */
export function killStaleInstances(): void {
  if (process.platform === 'win32') spawnSync('taskkill', ['/IM', 'mika-desktop.exe', '/F', '/T'], { stdio: 'ignore' })
  else {
    spawnSync('pkill', ['-9', '-x', 'mika-desktop'], { stdio: 'ignore' })
    // O plugin single-instance faz o app novo sair com código 0 se o anterior ainda segura o nome no D-Bus.
    for (let i = 0; i < 50 && spawnSync('pgrep', ['-x', 'mika-desktop'], { stdio: 'ignore' }).status === 0; i++) {
      spawnSync('sleep', ['0.1'])
    }
  }
}

/** Encerra o app e os filhos dele (WebView, MCP): árvore pelo `taskkill`, grupo de processos fora do Windows. */
export function killProcessTree(child: ChildProcess): void {
  if (!child.pid) {
    child.kill()
    return
  }
  if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(child.pid), '/F', '/T'], { stdio: 'ignore' })
  else
    try {
      process.kill(-child.pid, 'SIGKILL')
    } catch {
      child.kill('SIGKILL')
    }
}

export async function freePort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const server = createServer()
    server.unref()
    server.on('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      if (address === null || typeof address === 'string') {
        reject(new Error('sem porta livre'))
        return
      }
      const { port } = address
      server.close(() => resolvePort(port))
    })
  })
}

/**
 * CDP's `page.close()` kills only the WebView2 target; the Tauri window stays registered and the
 * next open re-shows it blank. Secondary windows close through the app's own `window.close()`.
 */
function closeLikeTheUser(page: Page): void {
  page.close = async () => {
    if (page.isClosed()) return
    const closed = page.waitForEvent('close')
    await page.evaluate(() => window.close()).catch(() => undefined)
    await closed
  }
}

/**
 * O primeiro `page.evaluate()` logo após `connectOverCDP` pega o app AINDA navegando (dev boot →
 * rota final) e explode com "Execution context was destroyed, most likely because of a
 * navigation" — `expect.poll()` NÃO engole essa exceção (propaga na hora, não espera o timeout),
 * então sem isto toda spec falharia em ~3s, sempre, de forma indistinguível de um boot quebrado.
 * Absorve aqui, uma vez, para as specs não precisarem de try/catch cada uma.
 */
export async function waitForStableExecutionContext(page: Page, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown
  // Um `evaluate` que funcionou uma vez não prova que a página parou de navegar nem que a ponte já
  // existe (`window.electronAPI` nasce depois do primeiro documento): exige a ponte e o MESMO
  // `timeOrigin` — que muda a cada navegação — em duas amostras seguidas.
  let previous: number | null = null
  while (Date.now() < deadline) {
    try {
      const sample = await page.evaluate(() => ({
        bridge: Boolean((window as unknown as { electronAPI?: unknown }).electronAPI),
        origin: performance.timeOrigin,
      }))
      if (sample.bridge && sample.origin === previous) return
      previous = sample.bridge ? sample.origin : null
    } catch (error) {
      lastError = error
      previous = null
    }
    await new Promise((r) => setTimeout(r, 250))
  }
  throw new Error(`contexto de execução da página nunca ficou estável em ${timeoutMs}ms (${String(lastError)})`)
}

type CdpTarget = { type: string; title: string; webSocketDebuggerUrl?: string }

export async function waitForCdpTarget(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json`)
      if (response.ok) {
        const targets = (await response.json()) as CdpTarget[]
        if (targets.some((target) => target.type === 'page' && target.webSocketDebuggerUrl)) return
      }
    } catch (error) {
      lastError = error
    }
    await new Promise((r) => setTimeout(r, 300))
  }
  throw new Error(`nenhum alvo CDP em http://127.0.0.1:${port}/json em ${timeoutMs}ms (${String(lastError)})`)
}

/**
 * Chama o gancho de teste Rust (`mika:e2e-hook`, `apps/desktop/src-tauri/src/modules/e2e_hooks.rs`)
 * de dentro da página real, pelo comando `mika_invoke` -> `NativeRegistry` que QUALQUER chamada de
 * produção usa. `window.electronAPI` (`contextBridge.exposeInMainWorld`, congelado em
 * `preload.module.ts`) NÃO tem `.invoke` genérico de propósito — adicionar um alargaria a
 * superfície de confiança do renderer para qualquer canal (violação de fronteira). Em vez disso,
 * isto chama `window.__TAURI_INTERNALS__.invoke` diretamente: o transporte IPC de baixo nível que
 * o próprio `@tauri-apps/api/core` usa por baixo (`ipcRenderer.invoke` em
 * `src/shared/tauri-renderer-runtime.ts` faz exatamente isto) e que o Tauri injeta sempre — ao
 * contrário de `window.__TAURI__` (o namespace de conveniência), que só existe com
 * `withGlobalTauri: true` (`tauri.conf.json` tem `false`). Nenhum canal novo fica alcançável por
 * isto: `mika_invoke` despacha para o `NativeRegistry`, e `e2e_hooks.rs` só reivindica
 * `mika:e2e-hook` com `MIKA_E2E=1` no processo (fail-closed em produção) — dupla trava com
 * `is_owner_surface`, doc no topo de `e2e_hooks.rs`. Só funciona a partir da janela
 * `main`/`settings`/`wizard` — uma `plugin-view` recebe o mesmo erro que em produção.
 */
export async function callE2eHook<T = unknown>(page: Page, op: string, extra: Record<string, unknown> = {}): Promise<T> {
  return page.evaluate(
    ([op, extra]) =>
      (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> }
        }
      ).__TAURI_INTERNALS__.invoke('mika_invoke', {
        channel: 'mika:e2e-hook',
        payload: { op, ...(extra as Record<string, unknown>) },
      }),
    [op, extra],
  ) as Promise<T>
}

/**
 * Amostra pixels REAIS da página composta (`page.screenshot({ omitBackground: true })`, o mesmo
 * compositor do Chromium/WebView2 que desenha a tela) em vez de capturar a janela do SO via
 * `xcap` (o gancho `capture-main-window` de `e2e_hooks.rs`).
 *
 * Substitui esse gancho — não é mais um atalho, é o único caminho que FUNCIONA para a própria
 * janela: `xcap::Window::all()` exclui a HWND do PRÓPRIO processo chamador por design
 * (`lp_dw_process_id == GetCurrentProcessId()` → `return false`, `windows/impl_window.rs` do crate
 * `xcap` 0.9.8 — o mesmo filtro que o WebRTC de onde o código foi portado usa para nunca deixar o
 * seletor de captura se capturar a si mesmo). Não é um problema de timing, de HWND filho do
 * WebView2 nem de plataforma — é uma garantia da API: nenhuma janela deste processo nunca aparece
 * em `all()`, então "achar a HWND certa e casar por id" (a primeira ideia óbvia) não teria o que
 * casar. `capture.rs` nunca bateu nisso porque a visão da Mika sempre captura a TELA ou OUTRAS
 * janelas, nunca a própria.
 *
 * `omitBackground: true` faz o Chromium/WebView2 não pintar branco por trás do DOM sem fundo
 * opaco — o PNG resultante carrega alfa real onde a página é transparente. Decodificado dentro da
 * PRÓPRIA página (canvas 2D via `data:` URL, sem taint de CORS) para não precisar de um decoder de
 * PNG no lado Node — mesma razão pela qual `capture-main-window` devolvia pixels já decodificados
 * em vez de um PNG cru (ver o cabeçalho de `e2e_hooks.rs`: nenhuma dependência de imagem existe em
 * `package.json`).
 */
export async function capturePagePixels(
  page: Page,
  samplePoints: Array<[number, number]>,
): Promise<{ width: number; height: number; samples: number[][] }> {
  const buffer = await page.screenshot({ omitBackground: true })
  return page.evaluate(
    async ([base64, points]) => {
      const img = new Image()
      img.src = `data:image/png;base64,${base64}`
      await img.decode()
      const canvas = document.createElement('canvas')
      canvas.width = img.naturalWidth
      canvas.height = img.naturalHeight
      const ctx = canvas.getContext('2d')
      if (!ctx) throw new Error('canvas 2d indisponível para decodificar a captura')
      ctx.drawImage(img, 0, 0)
      const samples = (points as number[][]).map(([x, y]) => {
        const px = Math.min(Math.max(x, 0), canvas.width - 1)
        const py = Math.min(Math.max(y, 0), canvas.height - 1)
        return Array.from(ctx.getImageData(px, py, 1, 1).data)
      })
      return { width: canvas.width, height: canvas.height, samples }
    },
    [buffer.toString('base64'), samplePoints] as const,
  )
}

export const test = base.extend<MikaFixtures & MikaOptions>({
  skin: [undefined, { option: true }],

  profile: async ({}, use) => {
    const profile = await createTempProfile()
    try {
      await use(profile)
    } finally {
      await profile.cleanup()
    }
  },

  llm: async ({}, use) => {
    const server = await startFakeOllama()
    try {
      await use(server)
    } finally {
      await server.close()
    }
  },

  wiki: async ({}, use) => {
    const server = await startFakeWiki()
    try {
      await use(server)
    } finally {
      await server.close()
    }
  },

  cdpPort: async ({}, use) => {
    await use(await freePort())
  },

  // Own budget: cold first launch is slower than 90s under load; keep it separate from the test timeout.
  app: [
    async ({ profile, llm, wiki, skin, cdpPort }, use) => {
      expect(
        existsSync(appBinaryPath),
        `${appBinaryPath} não existe. Rode "pnpm --filter @mika/desktop build:e2e" primeiro (NUNCA "cargo build --release" cru, e sem a feature e2e os ganchos de teste não existem — ver e2e/fixtures/app-binary.ts).`,
      ).toBe(true)

      killStaleInstances()

      const config: Record<string, unknown> = {
        setup: { wizardCompleted: true },
        legal: legalAccepted(),
        telemetry: { optInShown: true },
        // `MIKA_E2E_REAL_OLLAMA=<modelo>` aponta o app para o Ollama de verdade (smoke manual, nunca na
        // suíte determinística): o dublê continua de pé, só não é usado.
        llm: { provider: 'ollama', model: process.env.MIKA_E2E_REAL_OLLAMA ?? 'fake-model' },
        apiKeys: { ollamaBaseUrl: process.env.MIKA_E2E_REAL_OLLAMA ? 'http://127.0.0.1:11434' : llm.baseUrl },
        voice: { ttsEnabled: false },
        brain: { enabled: true },
        // `webEnrichment` é opt-in (default false em perfil novo): este perfil já o ligou, como o usuário
        // que consentiu — `memory-reaction` prova o lookup na Wikipédia (stub local).
        'person-catalog': { enabled: true, webEnrichment: true },
        mood: { enabled: true },
        vision: { enabled: false },
      }
      const skinId = skin ?? process.env.MIKA_E2E_SKIN
      if (skinId) {
        const model = AVATAR_MODELS.find((option) => option.id === skinId)
        if (!model) {
          const known = AVATAR_MODELS.map((option) => option.id).join(', ')
          throw new Error(`skin "${skinId}" não é skin nenhuma do catálogo. Conhecidas: ${known}`)
        }
        config.character = { modelPath: model.path }

        // Amostras Live2D são de terceiro e não vão no instalador (LICENSE-ASSETS.md) — o app
        // só as serve de `<userData>/live2d/<nome>/`, onde um usuário real as instala à mão.
        // Espelha esse fluxo aqui em vez de depender do bundle (que nunca as tem em release).
        if (!model.bundled) {
          const folder = model.path.split('/')[1]
          const source = resolve(__dirname, '../../src/renderer/public/live2d', folder)
          const dest = join(profile.userDataDir, 'live2d', folder)
          mkdirSync(dest, { recursive: true })
          cpSync(source, dest, { recursive: true })
        }
      }
      mkdirSync(profile.userDataDir, { recursive: true })
      writeFileSync(join(profile.userDataDir, 'config.json'), JSON.stringify(config, null, 2))

      const hostToken = randomUUID()
      const child: ChildProcess = spawn(appBinaryPath, [`--user-data-dir=${profile.userDataDir}`], {
        // Fora do Windows o grupo próprio deixa `killProcessTree` matar o app com os filhos.
        detached: process.platform !== 'win32',
        env: {
          ...process.env,
          MIKA_E2E: '1',
          MIKA_PROFILE_DIR: profile.userDataDir,
          // Redireciona `en.wikipedia.org`/`upload.wikimedia.org` pro stub local (`wiki` fixture) —
          // ver `guarded_fetch::e2e_wiki_override` (dupla trava com `MIKA_E2E=1`, nunca uma chave
          // de config; sem esta env var o backend bate na Wikipedia real como sempre).
          MIKA_E2E_WIKI_BASE_URL: wiki.baseUrl,
          // Google e OpenRouter têm URL fixa: com esta env var (dupla trava com `MIKA_E2E=1`, só no
          // binário `e2e`) o turno deles vai para o mesmo dublê local — ver `llm_provider::e2e_cloud_base`.
          MIKA_E2E_CLOUD_BASE_URL: llm.baseUrl,
          // E2E sem rede externa: o proxy aponta para uma porta fechada e só o loopback (LLM e Wikipédia
          // dublês) passa. Sem isto a preparação de modelos da tela final baixava ~790 MB de verdade e o
          // "Voltar" só destravava ao fim do download (ou nunca, num teste de 60 s).
          HTTP_PROXY: 'http://127.0.0.1:9',
          HTTPS_PROXY: 'http://127.0.0.1:9',
          NO_PROXY: '127.0.0.1,localhost,.localhost,tauri.localhost',
          ...(isHostMode
            ? {
                // Sem CDP fora do WebView2: o binário serve o bundle e o IPC em loopback (`e2e_host.rs`).
                MIKA_E2E_HOST_PORT: String(cdpPort),
                MIKA_E2E_HOST_TOKEN: hostToken,
                WEBKIT_DISABLE_DMABUF_RENDERER: '1',
                WEBKIT_DISABLE_COMPOSITING_MODE: '1',
              }
            : {
                WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`,
                // Sem isto o WebView2 usa `%LOCALAPPDATA%\<identificador>` — a MESMA pasta do app real e
                // de todas as execuções: o `localStorage` (última aba de Configurações, etc.) vazava de
                // um spec para o outro e a ordem decidia o resultado.
                WEBVIEW2_USER_DATA_FOLDER: join(profile.userDataDir, 'webview2'),
              }),
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      })
      child.stdout?.on('data', (d) => process.stdout.write(d))
      child.stderr?.on('data', (d) => process.stderr.write(d))

      let browser: Browser | undefined
      let host: HostSession | undefined
      let page: Page | undefined
      try {
        let context: BrowserContext
        if (isHostMode) {
          host = await connectHost(cdpPort, hostToken, child, 90_000)
          context = host.context
          page = host.firstPage()
        } else {
          await waitForCdpTarget(cdpPort, 90_000)
          browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`)
          const first = browser.contexts()[0]
          if (!first) throw new Error('connectOverCDP conectou mas não achou nenhum BrowserContext')
          context = first
          context.on('page', closeLikeTheUser)
          await expect
            .poll(() => context.pages().length, { timeout: 30_000, message: 'nenhuma Page apareceu no contexto CDP' })
            .toBeGreaterThan(0)
          page = context.pages()[0]
        }
        await page.waitForLoadState('domcontentloaded')
        await waitForStableExecutionContext(page, 30_000)

        const app: TauriShellApp = {
          firstWindow: async () => {
            if (!page) throw new Error('page não conectada')
            return page
          },
          waitForEvent: (async (event: 'window' | 'close') => {
            if (event === 'close') {
              if (child.exitCode !== null || child.killed) return
              return new Promise<void>((resolvePromise) => child.once('exit', () => resolvePromise()))
            }
            if (event !== 'window') throw new Error(`app.waitForEvent("${event}") não é suportado — só "window"/"close".`)
            if (host) {
              const newPage = await host.nextWindow(30_000)
              await newPage.waitForLoadState('domcontentloaded')
              await waitForStableExecutionContext(newPage, 30_000)
              return newPage
            }
            const before = new Set(context.pages())
            const newPage = await context.waitForEvent('page', { timeout: 30_000 })
            if (before.has(newPage)) {
              // Raced a page that already existed; wait for the actually-new one instead of misreporting it.
              return context.waitForEvent('page', { timeout: 30_000 })
            }
            await newPage.waitForLoadState('domcontentloaded')
            // `domcontentloaded` não basta numa janela nova: a ponte (`window.electronAPI`) nasce depois e a
            // página ainda pode navegar — os specs abriam a janela e chamavam `evaluate` antes disso.
            await waitForStableExecutionContext(newPage, 30_000)
            return newPage
          }) as TauriShellApp['waitForEvent'],
          evaluate: () => {
            throw new Error(
              'app.evaluate() não tem equivalente aqui — é main process do Electron (BrowserWindow/app/powerMonitor). ' +
                'Use callE2eHook(page, op, payload) — o gancho Rust `mika:e2e-hook` (MIKA_E2E=1, ver ' +
                'src-tauri/src/modules/e2e_hooks.rs) — ou, para inspecionar OUTRA janela, um segundo connectOverCDP.',
            )
          },
          process: () => child,
          close: async () => {
            try {
              await browser?.close()
              await host?.close()
            } catch {}
            killProcessTree(child)
          },
        }

        await use(app)
      } finally {
        try {
          await browser?.close()
          await host?.close()
        } catch {}
        if (child.exitCode === null && !child.killed) killProcessTree(child)
        killStaleInstances()
      }
    },
    { timeout: 90_000 },
  ],
})

export { expect }
