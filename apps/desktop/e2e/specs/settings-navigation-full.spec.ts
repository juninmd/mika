import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots/settings-nav')
const CONTROLS_SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')

function shotPath(name: string): string {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

function slug(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/gi, '-').replace(/^-+|-+$/g, '') || 'tab'
}

function standardControlsShotPath(name: string): string {
  return join(CONTROLS_SCREENSHOTS_DIR, `standard-controls-${name}.png`)
}

// Negative proof of the gold-standard UI (AGENTS.md §3.6): no settings panel may render a visible native
// <select>/checkbox/range — those are AuroraField/GlassSelect/GlassSlider/GlassToggle, which render
// button[role=switch]/[aria-haspopup=listbox], never the raw HTML element. SDK controls tag themselves
// with data-sdk-control; anything without the tag was hand-written on the screen.
async function assertNoRawControls(mainArea: import('@playwright/test').Locator, tabName: string): Promise<void> {
  const raw = {
    '<select> — use GlassSelect': 'select:visible',
    'checkbox — use GlassToggle': 'input[type="checkbox"]:visible',
    'role=switch — use GlassToggle': '[role="switch"]:not([data-sdk-control]):visible',
    'range — use GlassSlider': 'input[type="range"]:not([data-sdk-control]):visible',
    'input/textarea — use AuroraField': ':is(input, textarea):not([data-sdk-control]):not([type="file"]):not([type="hidden"]):visible',
  }
  for (const [what, selector] of Object.entries(raw)) {
    await expect(mainArea.locator(selector), `${tabName}: controle cru visível (${what})`).toHaveCount(0)
  }
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      getSettingsDeclarations: () => Promise<{ declarations: Array<{ tabId: string; tabLabel: string }> }>
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

test.describe('Settings — Navegação Completa de Menus e Submenus', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test.beforeAll(() => {
    mkdirSync(CONTROLS_SCREENSHOTS_DIR, { recursive: true })
  })

  test('Deve navegar e validar cada seção do menu lateral e cada submenu @standard-controls', async ({ app }) => {
    test.setTimeout(60_000)
    const mainPage = await app.firstWindow()
    await mainPage.waitForLoadState('domcontentloaded')
    await waitForBridge(mainPage)

    const settings = await openWindow(app, async () => {
      await clickDockButton(mainPage, 'Configurações')
    })
    await expect(settings.getByRole('navigation', { name: 'Navegação de configurações' })).toBeVisible()
    await waitForUiIdle(settings)

    const nav = settings.getByRole('navigation', { name: 'Navegação de configurações' })
    await expect(nav).toBeVisible()

    const navButtons = nav.locator('button')
    const navCount = await navButtons.count()
    expect(navCount).toBeGreaterThanOrEqual(7)

    for (let i = 0; i < navCount; i++) {
      const navBtn = navButtons.nth(i)
      const sectionName = (await navBtn.getAttribute('aria-label')) || `secao-${i}`

      await navBtn.click()
      await waitForUiIdle(settings)

      const subTabBar = settings.locator('[role="tablist"]')
      if (await subTabBar.isVisible().catch(() => false)) {
        const tabs = subTabBar.locator('button[role="tab"]')
        const tabCount = await tabs.count()

        for (let j = 0; j < tabCount; j++) {
          const tab = tabs.nth(j)

          await tab.click()
          await expect(tab).toHaveAttribute('aria-selected', 'true')
          await waitForUiIdle(settings)

          const mainArea = settings.locator('main')
          await expect(mainArea).toBeVisible()
          await expect(mainArea.getByText('Manual panel required')).toHaveCount(0)
          await assertNoRawControls(mainArea, `${sectionName} / ${await tab.textContent()}`)

          const tabLabel = ((await tab.textContent()) || `aba-${j}`).toLowerCase().replace(/[^a-z0-9]/gi, '-')
          await settings
            .screenshot({ path: standardControlsShotPath(`${slug(sectionName)}-${tabLabel}`), fullPage: true })
            .catch(() => {})
        }
      } else {
        const mainArea = settings.locator('main')
        await expect(mainArea).toBeVisible()
        await expect(mainArea.getByText('Manual panel required')).toHaveCount(0)
        await assertNoRawControls(mainArea, sectionName)

        await settings
          .screenshot({ path: standardControlsShotPath(slug(sectionName)), fullPage: true })
          .catch(() => {})
      }

      await settings.screenshot({
        path: shotPath(`menu-${sectionName.toLowerCase().replace(/[^a-z0-9]/gi, '-')}`),
        fullPage: true,
      })
    }

    await settings.close()
  })
})