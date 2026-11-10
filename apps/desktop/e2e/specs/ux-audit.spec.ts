import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { snapTop, waitForUiIdle } from '../fixtures/settled'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'

const PHASE = process.env.UX_SHOT_PHASE ?? 'before'
const SHOTS_DIR = join(process.cwd(), 'e2e/screenshots/ux', PHASE)

function shotPath(name: string) {
  return join(SHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      getSettingsDeclarations: () => Promise<{
        declarations: Array<{ tabId: string; tabLabel: string; groups: Array<{ component?: string }> }>
      }>
      saveSetting: (k: string, v: unknown) => Promise<unknown>
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

test.describe(`UX audit capture (${PHASE})`, () => {
  test.beforeAll(() => {
    mkdirSync(SHOTS_DIR, { recursive: true })
  })

  test('shell surfaces', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    await enableShellChrome(p)

    await p.screenshot({ path: shotPath('shell-dock'), fullPage: true })

    await p.keyboard.press('Control+k')
    await expect(p.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible({ timeout: 5000 })
    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('shell-command-palette'), fullPage: true })
    await p.keyboard.press('Escape')

    const input = await openComposer(p)
    await input.fill('mika, me conta uma curiosidade sobre jogos')
    await expect(input).toHaveValue('mika, me conta uma curiosidade sobre jogos')
    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('shell-composer'), fullPage: true })

    // Envio de mensagem pelo chat como usuário
    await p.keyboard.press('Enter')
    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('shell-chat-sent'), fullPage: true })

    // Teste de clique com botão direito para abrir menu de contexto da Mika
    await p.mouse.click(200, 300, { button: 'right' })
    const contextMenu = p.getByRole('menu')
    await expect(contextMenu).toBeVisible()
    await waitForUiIdle(p)
    const menuBox = await contextMenu.boundingBox()
    const viewport = p.viewportSize()
    if (menuBox && viewport) {
      expect(menuBox.x).toBeGreaterThanOrEqual(0)
      expect(menuBox.y).toBeGreaterThanOrEqual(0)
      expect(menuBox.x + menuBox.width).toBeLessThanOrEqual(viewport.width)
      expect(menuBox.y + menuBox.height).toBeLessThanOrEqual(viewport.height)
    }
    await p.screenshot({ path: shotPath('shell-context-menu'), fullPage: true })
    await p.keyboard.press('Escape')

    // Teste de arrastar janela (drag handle no topo)
    await p.mouse.move(200, 5)
    await p.mouse.down()
    await p.mouse.move(250, 15)
    await p.mouse.up()
    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('shell-after-drag'), fullPage: true })
  })

  test('settings surfaces', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const { declarations } = await p.evaluate(() =>
      (window as unknown as BridgeWindow).electronAPI!.runtime.getSettingsDeclarations(),
    )

    const seen = new Set<string>()
    const unique = declarations.filter((d) => {
      if (seen.has(d.tabId)) return false
      seen.add(d.tabId)
      return true
    })

    const settings = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })
    await expect(settings.getByLabel('Abrir paleta de comandos')).toBeVisible()
    await waitForUiIdle(settings)
    await snapTop(settings, shotPath('settings-default'))

    for (const decl of unique) {
      await settings.getByLabel('Abrir paleta de comandos').click()
      const search = settings.getByLabel('Buscar configurações')
      await expect(search).toBeVisible()
      await search.fill(decl.tabLabel)
      await settings.keyboard.press('Enter')
      await expect(search).toHaveCount(0)
      await waitForUiIdle(settings)
      await snapTop(settings, shotPath(`settings-${decl.tabId}`))
    }

    await settings.close()
  })

  test('wizard surfaces', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const wizard = await openWindow(app, async () => {
      await p.evaluate(() =>
        (window as unknown as BridgeWindow).electronAPI!.runtime.saveSetting('setup.wizardCompleted', false),
      )
    })
    await expect(wizard.getByText('Onboarding da Mika')).toBeVisible({ timeout: 10_000 })

    const steps = [
      { shot: 'wizard-01-language', marker: 'Etapa 1 de 8', next: 'Continuar' },
      { shot: 'wizard-02-avatar', marker: 'Etapa 2 de 8', next: 'Continuar' },
      { shot: 'wizard-03-llm', marker: 'Etapa 3 de 8', next: 'Continuar' },
      { shot: 'wizard-04-tts', marker: 'Etapa 4 de 8', next: 'Continuar' },
      { shot: 'wizard-05-stt', marker: 'Etapa 5 de 8', next: 'Continuar' },
      { shot: 'wizard-06-embedding', marker: 'Etapa 6 de 8', next: 'Continuar' },
      { shot: 'wizard-07-privacy', marker: 'Etapa 7 de 8', next: 'Salvar escolhas e continuar' },
      { shot: 'wizard-08-identity', marker: 'Etapa 8 de 8', next: 'Concluir setup' },
    ] as const

    for (const step of steps) {
      await expect(wizard.getByText(step.marker)).toBeVisible()
      await waitForUiIdle(wizard)
      await snapTop(wizard, shotPath(step.shot))
      if (step.shot === 'wizard-03-llm') {
        // O E2E sobe com o Ollama dublê (`fake-model`); o catálogo que o usuário vê numa instalação
        // nova é o dos provedores de nuvem. Mostra os modelos padrão de cada um e volta ao Ollama.
        for (const provider of ['Google AI (Gemini)', 'OpenRouter']) {
          await wizard.getByText(provider, { exact: true }).click()
          await waitForUiIdle(wizard)
          await snapTop(wizard, shotPath(`wizard-03-llm-${provider.split(' ')[0]?.toLowerCase()}`))
        }
        await wizard.getByText('Ollama (local)', { exact: true }).click()
        await waitForUiIdle(wizard)
      }
      await wizard.getByRole('button', { name: step.next, exact: true }).click()
    }

    await expect(wizard.getByText('Mika pronta')).toBeVisible()
    await waitForUiIdle(wizard)
    await snapTop(wizard, shotPath('wizard-09-done'))
  })
})
