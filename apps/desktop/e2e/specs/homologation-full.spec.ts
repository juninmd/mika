import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')

function shotPath(name: string): string {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      getHealth: () => Promise<{ status?: string; capabilities?: Record<string, unknown> }>
      getSettingsDeclarations: () => Promise<{ declarations: Array<{ tabId: string; tabLabel: string }> }>
      getPluginUi: () => Promise<{ views?: unknown[] }>
      saveSetting: (k: string, v: unknown) => Promise<unknown>
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

async function getHealth(page: Page) {
  return page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getHealth())
}

async function getSettingsDeclarations(page: Page) {
  return page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getSettingsDeclarations())
}

async function getPluginUi(page: Page) {
  return page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getPluginUi())
}

async function saveSetting(page: Page, key: string, value: unknown) {
  return page.evaluate(
    ({ k, v }: { k: string; v: unknown }) => (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting(k, v),
    { k: key, v: value },
  )
}

async function navigateSettingsTab(settings: Page, query: string) {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

test.describe('Homologação 100% — Web Interfaces', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('1. Desktop Shell', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const health = await getHealth(p)
    expect(health).toHaveProperty('status')
    expect(health?.capabilities).toHaveProperty('plugins')

    writeFileSync(join(SCREENSHOTS_DIR, 'health-report.json'), JSON.stringify(health, null, 2))
    await p.screenshot({ path: shotPath('01-desktop-shell'), fullPage: true })
  })

  test('2. Settings — All Tabs', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const declarations = (await getSettingsDeclarations(p))?.declarations ?? []
    expect(declarations.length).toBeGreaterThan(0)

    const seen = new Set<string>()
    const tabs: Array<{ label: string; id: string }> = []
    for (const d of declarations) {
      if (!seen.has(d.tabId)) {
        seen.add(d.tabId)
        tabs.push({ label: d.tabLabel, id: d.tabId })
      }
    }

    const settings = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })

    for (const { label, id } of tabs) {
      await navigateSettingsTab(settings, label)
      await waitForUiIdle(settings)
      await settings.screenshot({ path: shotPath(`settings-${id.replace(/[^a-z0-9]/gi, '-')}`), fullPage: true })
      await expect(settings.getByText('Manual panel required')).toHaveCount(0)
      await expect(settings.getByRole('heading').first()).toBeVisible()
    }

    await settings.close()
  })

  test('6. Plugin View declarations', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const catalog = await getPluginUi(p)
    expect(catalog?.views).toBeDefined()
    expect(Array.isArray(catalog?.views)).toBe(true)

    await p.screenshot({ path: shotPath('plugin-declarations'), fullPage: true })
  })

  test('7. Wizard flow', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const winPromise = app.waitForEvent('window')
    await saveSetting(p, 'setup.wizardCompleted', false)
    const wizardPage = await winPromise
    await wizardPage.waitForLoadState('domcontentloaded')

    await expect(wizardPage.getByText('Onboarding da Mika')).toBeVisible({ timeout: 5000 })
    await wizardPage.screenshot({ path: shotPath('wizard-language-step'), fullPage: true })
    await wizardPage.close()
  })

  test('8. Command Palette', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await enableShellChrome(p)
    await p.keyboard.press('Control+k')
    await expect(p.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible()
    await p.screenshot({ path: shotPath('command-palette'), fullPage: true })
    await p.keyboard.press('Escape')
  })

  test('9. Tool Execution View', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await enableShellChrome(p)
    await p.keyboard.press('Control+k')
    await expect(p.getByPlaceholder(/Buscar|comando|Digite/i).first()).toBeVisible()
    await p.screenshot({ path: shotPath('tool-execution'), fullPage: true })

    await expect(p.getByText('memory console', { exact: true })).toBeVisible()
    await p.keyboard.press('Escape')
  })
})
