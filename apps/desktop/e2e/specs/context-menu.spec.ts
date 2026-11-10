import { expect, test } from '../fixtures/tauri-app'
import { artifactPath } from '../fixtures/artifact'
import { enableShellChrome } from '../fixtures/shell-chrome'

test('menu de contexto da Mika abre com clique direito e responde a comandos', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await expect
    .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
    .toBe(true)

  await enableShellChrome(page)

  // Dispara o evento de contextmenu diretamente na raiz da aplicação
  await page.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 200, clientY: 200 })

  const menu = page.locator('[role="menu"]')
  await expect(menu).toBeVisible({ timeout: 5000 })
  await page.screenshot({ path: artifactPath('context-menu-open.png') })

  // Tecla Escape fecha o menu
  await page.keyboard.press('Escape')
  await expect(menu).toHaveCount(0)

  // Reabre via contextmenu
  await page.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 250, clientY: 250 })
  await expect(menu).toBeVisible({ timeout: 5000 })

  // Clica no primeiro item do menu
  const firstItem = menu.locator('[role="menuitem"]').first()
  await expect(firstItem).toBeVisible()
  await firstItem.click()

  // O menu se fecha após a seleção
  await expect(menu).toHaveCount(0)
})
