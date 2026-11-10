import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { expect, test } from '../fixtures/tauri-app'
import { enableShellChrome } from '../fixtures/shell-chrome'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')
function shotPath(name: string) {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      getHealth: () => Promise<unknown>
      saveSetting: (k: string, v: unknown) => Promise<unknown>
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

const WIZARD_STEPS = [
  { shot: 'wizard-step-01-language', marker: 'Etapa 1 de 8', next: 'Continuar' },
  { shot: 'wizard-step-02-avatar', marker: 'Etapa 2 de 8', next: 'Continuar' },
  { shot: 'wizard-step-03-llm', marker: 'Etapa 3 de 8', next: 'Continuar' },
  {
    shot: 'wizard-step-04-tts',
    marker: 'Etapa 4 de 8',
    pick: 'Supertonic (embutida, padrão)',
    next: 'Continuar',
  },
  { shot: 'wizard-step-05-stt', marker: 'Etapa 5 de 8', pick: /Nemotron local/, next: 'Continuar' },
  { shot: 'wizard-step-06-embedding', marker: 'Etapa 6 de 8', next: 'Continuar' },
  { shot: 'wizard-step-07-privacy', marker: 'Etapa 7 de 8', next: 'Salvar escolhas e continuar' },
  { shot: 'wizard-step-08-identity', marker: 'Etapa 8 de 8', next: 'Concluir setup' },
] as const

test.describe('Homologação — Capture ALL Surfaces', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('01 - Desktop Shell + Health', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    const health = await p.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getHealth())
    writeFileSync(join(SCREENSHOTS_DIR, 'health-report.json'), JSON.stringify(health, null, 2))
    await p.screenshot({ path: shotPath('01-desktop-shell'), fullPage: true })
  })

  test('04 - Wizard Flow (All Steps)', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const winPromise = app.waitForEvent('window')
    await p.evaluate(() =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting('setup.wizardCompleted', false),
    )
    const wizardPage = await winPromise
    await wizardPage.waitForLoadState('domcontentloaded')
    await expect(wizardPage.getByText('Onboarding da Mika')).toBeVisible({ timeout: 10_000 })

    for (const step of WIZARD_STEPS) {
      await expect(wizardPage.getByText(step.marker)).toBeVisible()
      if ('pick' in step && step.pick) {
        await wizardPage.getByRole('button', { name: step.pick }).first().click()
      }
      await wizardPage.screenshot({ path: shotPath(step.shot), fullPage: true })
      await wizardPage.getByRole('button', { name: step.next, exact: true }).click()
    }

    await expect(wizardPage.getByText('Mika pronta')).toBeVisible()
    // O botão de entrada fica "Preparando componentes..." até o lote de ativos locais acabar:
    // VAD, STT nemotron (~790 MB), voz local e embedding baixam aqui de verdade. É o passo final
    // do onboarding — sem esperar o lote, o spec testaria um estado que o usuário não vê.
    await expect(wizardPage.getByRole('button', { name: 'Entrar no desktop', exact: true })).toBeVisible({
      timeout: 900_000,
    })
    await wizardPage.screenshot({ path: shotPath('wizard-step-09-done'), fullPage: true })
    await wizardPage.getByRole('button', { name: 'Entrar no desktop', exact: true }).click()
  })

  test('05 - All Overlay Surfaces', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await enableShellChrome(p)

    await p.keyboard.press('Control+k')
    await expect(p.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible()
    await p.screenshot({ path: shotPath('overlay-command-palette'), fullPage: true })

    await expect(p.getByText('get system info')).toBeVisible()
    await p.screenshot({ path: shotPath('overlay-tools-panel'), fullPage: true })
    await p.keyboard.press('Escape')

    await openComposer(p)
    await p.screenshot({ path: shotPath('overlay-chat-input'), fullPage: true })
    await p.keyboard.press('Escape')

    await p.screenshot({ path: shotPath('overlay-dock-buttons'), fullPage: true })
  })
})
