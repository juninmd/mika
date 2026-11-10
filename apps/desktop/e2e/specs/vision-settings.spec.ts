import { expect, test } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'
import { artifactPath } from '../fixtures/artifact'

test('vision settings panel renders all sections', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await expect(settings.getByLabel('Abrir paleta de comandos')).toBeVisible()
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill('Vision')
  await settings.keyboard.press('Enter')

  await expect(settings.getByRole('heading', { name: /Visão/i }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
  await expect(settings.getByText(/Visão|qualidade|intervalo|modo/i).first()).toBeVisible()
  await settings.screenshot({ path: artifactPath('vision-settings.png') })
})
