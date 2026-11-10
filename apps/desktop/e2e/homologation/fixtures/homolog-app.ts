/**
 * Sucessor da versão Electron (`electron.launch`) — dirige o binário TAURI via CDP, reusando as
 * mesmas primitivas de `../../fixtures/tauri-app.ts` (que essa suíte NÃO possui: aqui). A diferença
 * frente a `tauri-app.ts` é de propósito, não de mecânica: homologação usa um perfil PERSISTENTE
 * (`HOMOLOG_PROFILE`, não um temp dir descartado) e fala com o Ollama local de verdade, nunca com o
 * `fake-ollama-server` — homologar contra um dublê não homologa nada (ver README.md).
 *
 * `app.evaluate()` continua sem equivalente sob Tauri (é main process do Electron). O único spec
 * desta suíte que dependia disso, `probe-defaults.spec.ts`, sondava `globalThis.e2e` (o container
 * de DI que `src/main/bootstrap.ts` expõe só no processo Electron) — não há comando Rust equivalente,
 * então essa sonda é um GAP DE MIGRAÇÃO real, não um bug de harness.
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { chromium, test as base, type Browser, type BrowserContext, expect, type Page } from '@playwright/test'
import {
  appBinaryPath,
  killStaleInstances,
  waitForCdpTarget,
  waitForStableExecutionContext,
} from '../../fixtures/tauri-app'
import { legalAccepted } from '../../fixtures/legal-accepted'

const DEFAULT_PROFILE = join(process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'), 'Mika', 'desktop-homolog')

export const HOMOLOG_PROFILE = process.env.MIKA_HOMOLOG_PROFILE ?? DEFAULT_PROFILE

const OLLAMA_BASE_URL = process.env.MIKA_HOMOLOG_OLLAMA ?? 'http://127.0.0.1:11434'

/** Trocável para homologar o MESMO roteiro contra outro provedor sem editar spec nenhuma. */
const PROVIDER = process.env.MIKA_HOMOLOG_PROVIDER ?? 'ollama'

/** `qwen3.5:4b` — 3,4 GB, com `tools` e `vision` na mesma pesagem, que é o que a Mika exige. */
const DEFAULT_MODEL: Record<string, string> = {
  ollama: 'qwen3.5:4b',
  openrouter: 'google/gemma-4-31b-it:free',
}

const CHAT_MODEL = process.env.MIKA_HOMOLOG_MODEL ?? DEFAULT_MODEL[PROVIDER] ?? DEFAULT_MODEL.ollama

const VISION_PROVIDER = process.env.MIKA_HOMOLOG_VISION_PROVIDER ?? PROVIDER
const VISION_MODEL = process.env.MIKA_HOMOLOG_VISION_MODEL ?? CHAT_MODEL

/** Cada provedor lê o modelo de uma chave própria; escrever só `llm.model` deixa o outro no default. */
const MODEL_KEY: Record<string, string> = { ollama: 'ollamaModel', openrouter: 'openrouterModel' }

async function freePort(): Promise<number> {
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

export type ToolResult = { success?: boolean; data?: unknown; error?: string }

/** Casca do app só para o `probe-defaults.spec.ts` reportar o gap com uma mensagem clara. */
export type HomologShellApp = {
  evaluate: (..._args: unknown[]) => never
}

type HomologFixtures = {
  app: HomologShellApp
  window: Page
  runTool: (name: string, args?: Record<string, unknown>) => Promise<ToolResult>
}

type LaunchedApp = { page: Page; close: () => Promise<void> }

async function waitForBridgePage(context: BrowserContext): Promise<Page> {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    for (const candidate of context.pages()) {
      const hasBridge = await candidate
        .evaluate(() => Boolean((window as unknown as { electronAPI?: { runtime?: unknown } }).electronAPI?.runtime))
        .catch(() => false)
      if (hasBridge) return candidate
    }
    await new Promise((r) => setTimeout(r, 300))
  }
  throw new Error('nenhuma Page expõe window.electronAPI.runtime em 30s')
}

/**
 * Sobe o binário Tauri com o perfil PERSISTENTE de homologação e conecta por CDP — mesma mecânica
 * de `tauri-app.ts`, fatorada uma vez aqui porque `app` e `window` precisam dela cada um por si
 * (nunca as duas juntas: nenhum spec desta suíte pede `app` e `window` no mesmo teste).
 */
async function launchApp(): Promise<LaunchedApp> {
  expect(
    existsSync(appBinaryPath),
    `${appBinaryPath} não existe. Rode "pnpm --filter @mika/desktop build:e2e" primeiro (NUNCA "cargo build --release" cru, e sem a feature e2e os ganchos de teste não existem — ver e2e/fixtures/app-binary.ts).`,
  ).toBe(true)

  writeHomologConfig(HOMOLOG_PROFILE)
  killStaleInstances()

  const cdpPort = await freePort()
  const child: ChildProcess = spawn(appBinaryPath, [`--user-data-dir=${HOMOLOG_PROFILE}`], {
    env: {
      ...process.env,
      MIKA_E2E: '1',
      MIKA_PROFILE_DIR: HOMOLOG_PROFILE,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`,
      // Dados do WebView2 no perfil de homologação, não em `%LOCALAPPDATA%` (a pasta do app real).
      WEBVIEW2_USER_DATA_FOLDER: join(HOMOLOG_PROFILE, 'webview2'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  child.stdout?.on('data', (d) => process.stdout.write(d))
  child.stderr?.on('data', (d) => process.stderr.write(d))

  const closeAll = async (browser: Browser | undefined) => {
    try {
      await browser?.close()
    } catch {}
    if (child.exitCode === null && !child.killed) {
      if (child.pid) spawnSync('taskkill', ['/PID', String(child.pid), '/F', '/T'], { stdio: 'ignore' })
      else child.kill()
    }
    killStaleInstances()
  }

  let browser: Browser | undefined
  try {
    await waitForCdpTarget(cdpPort, 90_000)
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`)
    const context = browser.contexts()[0]
    if (!context) throw new Error('connectOverCDP conectou mas não achou nenhum BrowserContext')
    await expect
      .poll(() => context.pages().length, { timeout: 30_000, message: 'nenhuma Page apareceu no contexto CDP' })
      .toBeGreaterThan(0)
    // `pages()[0]` pode ser uma janela sem a ponte (mais de um webview): escolhe a que expõe o runtime.
    const page = await waitForBridgePage(context)
    await page.waitForLoadState('domcontentloaded')
    await waitForStableExecutionContext(page, 30_000)

    const capturedBrowser = browser
    return { page, close: () => closeAll(capturedBrowser) }
  } catch (error) {
    await closeAll(browser)
    throw error
  }
}

function writeHomologConfig(userDataDir: string): void {
  mkdirSync(userDataDir, { recursive: true })
  const configPath = join(userDataDir, 'config.json')

  if (process.env.MIKA_HOMOLOG_FRESH === '1') return

  let existing: Record<string, unknown> = {}
  if (existsSync(configPath)) {
    try {
      existing = JSON.parse(readFileSync(configPath, 'utf8')) as Record<string, unknown>
    } catch {
      existing = {}
    }
  }

  const previousApiKeys = (existing.apiKeys as Record<string, unknown> | undefined) ?? {}

  const config: Record<string, unknown> = {
    ...existing,
    setup: { wizardCompleted: true },
    legal: legalAccepted(),
    telemetry: { optInShown: true },
    llm: {
      ...((existing.llm as Record<string, unknown> | undefined) ?? {}),
      provider: PROVIDER,
      model: CHAT_MODEL,
      [MODEL_KEY[PROVIDER] ?? 'ollamaModel']: CHAT_MODEL,
      // Embedding fica sempre local: é o único ponto do roteiro que roda a cada memória gravada, e
      // pagá-lo por token não prova nada que o Ollama já não prove.
      embeddingProvider: 'ollama',
      embeddingModel: 'nomic-embed-text',
    },
    apiKeys: { ...previousApiKeys, ollamaBaseUrl: OLLAMA_BASE_URL },
    vision: {
      ...((existing.vision as Record<string, unknown> | undefined) ?? {}),
      // Ligada: com visão desligada as tools respondem "disabled" e a homologação vira enfeite.
      enabled: process.env.MIKA_HOMOLOG_NO_VISION === '1' ? false : true,
      provider: VISION_PROVIDER,
      model: VISION_MODEL,
    },
    voice: { ttsEnabled: false },
    // Sem isto o adaptador de tela recusa a captura e `vision_capture`/`vision_read` respondem
    // "Captura de Tela está desligada em Privacidade" — verde que não prova nada.
    privacy: { ...((existing.privacy as Record<string, unknown> | undefined) ?? {}), screenCapture: true },
    brain: { enabled: true },
    // Ligado de propósito: com `mcp.enabled` no padrão (false) a feature nem ativa, e o
    // `testConnection` só conseguia responder "não está ativa" — não homologava nada.
    mcp: { ...((existing.mcp as Record<string, unknown> | undefined) ?? {}), enabled: true },
    mood: { enabled: true },
    // Busca web e consulta de pessoas na web são opt-in (default false em perfil novo): a homologação
    // as liga de propósito, como o usuário que consentiu, para continuar exercitando o caminho real.
    browser: { ...((existing.browser as Record<string, unknown> | undefined) ?? {}), enabled: true },
    'person-catalog': { enabled: true, webEnrichment: true },
  }

  writeFileSync(configPath, JSON.stringify(config, null, 2))
}

export const test = base.extend<HomologFixtures>({
  app: [
    async ({}, use) => {
      const { close } = await launchApp()
      try {
        const app: HomologShellApp = {
          evaluate: () => {
            throw new Error(
              'app.evaluate() não tem equivalente sob Tauri — é main process do Electron. Este spec sondava ' +
                'globalThis.e2e (container de DI só exposto em src/main/bootstrap.ts, Electron); não há comando ' +
                'Rust equivalente hoje. GAP DE MIGRAÇÃO, não bug de harness.',
            )
          },
        }
        await use(app)
      } finally {
        await close()
      }
    },
    { timeout: 90_000 },
  ],

  window: async ({}, use) => {
    const { page, close } = await launchApp()
    try {
      await use(page)
    } finally {
      await close()
    }
  },

  runTool: async ({ window }, use) => {
    await window.evaluate(() => {
      const runtime = (
        window as unknown as {
          electronAPI: {
            runtime: {
              onToolApprovalRequest: (cb: (req: { toolCallId: string; toolName: string }) => void) => void
              sendToolApprovalResponse: (id: string, approved: boolean, scope?: 'once') => void
            }
          }
        }
      ).electronAPI.runtime
      const approved: string[] = []
      ;(window as unknown as { __homologApprovals: string[] }).__homologApprovals = approved
      runtime.onToolApprovalRequest((req) => {
        approved.push(req.toolName)
        runtime.sendToolApprovalResponse(req.toolCallId, true, 'once')
      })
    })

    await use(async (name, args = {}) => {
      return (await window.evaluate(
        async ({ name, args }) => {
          const runtime = (
            window as unknown as {
              electronAPI: { runtime: { executeTool: (n: string, a: Record<string, unknown>) => Promise<unknown> } }
            }
          ).electronAPI.runtime
          try {
            return (await runtime.executeTool(name, args)) as Record<string, unknown>
          } catch (error) {
            return { success: false, error: error instanceof Error ? error.message : String(error) }
          }
        },
        { name, args },
      )) as ToolResult
    })
  },
})

export { expect }
