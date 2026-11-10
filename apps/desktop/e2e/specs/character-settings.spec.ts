import { expect, test } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'

async function openSettingsFlow(page: ReturnType<(typeof test)['info']> extends never ? never : any) {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  return settings
}

async function navigatePalette(settings: any, query: string) {
  await expect(settings.getByLabel('Abrir paleta de comandos')).toBeVisible()
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
}

test('character identity panel renders core fields', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  const settings = await openSettingsFlow(page)
  await navigatePalette(settings, 'Personagem')
  await expect(settings.getByRole('heading', { name: /Personagem/i }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
})

test('behavior panel renders personality settings', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  const settings = await openSettingsFlow(page)
  await navigatePalette(settings, 'Personagem')
  await expect(settings.getByRole('heading', { name: /Personagem/i }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)

  await expect(settings.getByText(/Comportamento|Sarcasmo|Caos|Tsundere/i).first()).toBeVisible()

  // securityLevel 'strict' sanitiza o SVG: o fluxo tem de continuar desenhado e com os rótulos.
  const diagram = settings.locator('.mermaid-diagram')
  await diagram.scrollIntoViewIfNeeded()
  await expect(diagram.locator('svg')).toBeVisible()
  await expect(diagram).toContainText('LLM decide')
  await expect(settings.getByText('Diagram error')).toHaveCount(0)
  await diagram.screenshot({ path: 'e2e/screenshots/character-behavior-flow.png' })
})
