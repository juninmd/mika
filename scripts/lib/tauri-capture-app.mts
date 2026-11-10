import { type ChildProcess, spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { join } from 'node:path'
import { type Browser, chromium, type Page } from '@playwright/test'
import { appBinaryPath } from '../../apps/desktop/e2e/fixtures/app-binary.ts'

/** Release binary, the same one the E2E suite drives (resolved once in `e2e/fixtures/app-binary.ts`). */
export const APP_BINARY = appBinaryPath

const BOOT_TIMEOUT_MS = 90_000

export type CaptureApp = {
  page: Page
  pages: () => Page[]
  setSize: (width: number, height: number) => Promise<void>
  close: () => Promise<void>
}

function freePort(): Promise<number> {
  return new Promise((done, fail) => {
    const server = createServer()
    server.on('error', fail)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => (address && typeof address === 'object' ? done(address.port) : fail(new Error('sem porta'))))
    })
  })
}

async function connect(port: number): Promise<Browser> {
  const deadline = Date.now() + BOOT_TIMEOUT_MS
  while (Date.now() < deadline) {
    const targets = await fetch(`http://127.0.0.1:${port}/json`)
      .then((r) => (r.ok ? (r.json() as Promise<{ type: string }[]>) : []))
      .catch(() => [])
    if (targets.some((t) => t.type === 'page')) return chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    await new Promise((r) => setTimeout(r, 300))
  }
  throw new Error(`o app não abriu alvo CDP em ${BOOT_TIMEOUT_MS / 1000}s`)
}

/** Waits for the exit too: WebView2 keeps the profile locked until the process tree is gone. */
async function kill(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null) return
  const exited = new Promise((done) => child.once('exit', done))
  // `taskkill` só existe no Windows; fora dele o app ficava vivo (e com o perfil travado) depois do script.
  if (process.platform !== 'win32') child.kill('SIGKILL')
  else if (child.pid) spawnSync('taskkill', ['/PID', String(child.pid), '/F', '/T'], { stdio: 'ignore' })
  await Promise.race([exited, new Promise((done) => setTimeout(done, 10_000))])
}

/** Boots the Tauri app on a seeded profile with CDP on (WebView2), for scripted captures. */
export async function launchCaptureApp(userDataDir: string, config: Record<string, unknown>): Promise<CaptureApp> {
  if (!existsSync(APP_BINARY)) {
    throw new Error(`${APP_BINARY} não existe. Rode antes: cd apps/desktop && npx tauri build --no-bundle`)
  }
  spawnSync('taskkill', ['/IM', 'mika-desktop.exe', '/F', '/T'], { stdio: 'ignore' })
  mkdirSync(userDataDir, { recursive: true })
  writeFileSync(join(userDataDir, 'config.json'), JSON.stringify(config, null, 2))

  const port = await freePort()
  const child = spawn(APP_BINARY, [`--user-data-dir=${userDataDir}`], {
    env: {
      ...process.env,
      MIKA_E2E: '1',
      MIKA_PROFILE_DIR: userDataDir,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
    stdio: 'ignore',
  })
  try {
    const browser = await connect(port)
    const context = browser.contexts()[0]
    if (!context) throw new Error('CDP conectou sem BrowserContext')
    const page = context.pages()[0] ?? (await context.waitForEvent('page'))
    await page.waitForLoadState('domcontentloaded')
    // The first CDP page can still be navigating to the app route; the IPC global is the ready signal.
    await page.waitForFunction(() => '__TAURI_INTERNALS__' in window, undefined, { timeout: BOOT_TIMEOUT_MS })
    // tsx wraps named inner functions in `__name`, a helper that does not exist in the page.
    await page.evaluate('globalThis.__name = (fn) => fn')
    return {
      page,
      pages: () => context.pages(),
      // `window-set-size` is the E2E hook (`e2e_hooks.rs`, MIKA_E2E=1 only) — Tauri has no BrowserWindow here.
      setSize: async (width, height) => {
        await page.evaluate(
          (size) =>
            (
              window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> } }
            ).__TAURI_INTERNALS__.invoke('mika_invoke', {
              channel: 'mika:e2e-hook',
              payload: { op: 'window-set-size', ...size },
            }),
          { width, height },
        )
      },
      close: async () => {
        await browser.close().catch(() => undefined)
        await kill(child)
      },
    }
  } catch (error) {
    await kill(child)
    throw error
  }
}
