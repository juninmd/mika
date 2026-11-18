import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { expect, test } from '../fixtures/tauri-app'

// The edge dock used to be reachable only through Settings → Window (style + mini toggle). The avatar's
// context menu now carries "Dock de borda" next to "Modo mini": one click docks her, one click brings her back.
type ShellApi = { electronAPI: { runtime: { getSettingsConfig(): Promise<{ config: { app?: { miniModeStyle?: string } } }> } } }

const savedStyle = (page: Page) =>
  page.evaluate(async () => (await (window as never as ShellApi).electronAPI.runtime.getSettingsConfig()).config.app?.miniModeStyle)

const docked = (page: Page) => page.evaluate(() => document.body.dataset.edgeDock === 'true')

async function openMenu(page: Page) {
  await page.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 220, clientY: 220 })
  const menu = page.locator('[role="menu"]')
  await expect(menu).toBeVisible({ timeout: 5000 })
  return menu
}

test('context menu docks Mika to the screen edge and brings her back', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await enableShellChrome(page)

  const menu = await openMenu(page)
  const dockItem = menu.getByRole('menuitemcheckbox', { name: /Dock de borda/ })
  await expect(dockItem).toHaveAttribute('aria-checked', 'false')
  await page.screenshot({ path: artifactPath('edge-dock-menu.png') })
  await dockItem.click()

  await expect.poll(() => docked(page), { timeout: 10_000 }).toBe(true)
  expect(await savedStyle(page)).toBe('dock-right')
  await page.screenshot({ path: artifactPath('edge-dock-on.png') })

  // Leaving mini mode brings the full window back (the dock has its own expand button, same channel).
  await page.evaluate(() => {
    const api = (window as never as { electronAPI: { shell: { window: { setMiniMode(on: boolean): Promise<unknown> } } } })
      .electronAPI.shell.window
    return api.setMiniMode(false)
  })
  await expect.poll(() => docked(page), { timeout: 10_000 }).toBe(false)

  writeArtifact('edge-dock-menu', { dockedFromMenu: true, savedStyle: await savedStyle(page), dockedAfterLeave: await docked(page) })
})
