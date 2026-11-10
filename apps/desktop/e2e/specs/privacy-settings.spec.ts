import { expect, test } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'

test('Privacy settings panel renders the memory transparency card', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill('privacidade')
  await settings.keyboard.press('Enter')

  await expect(settings.getByRole('heading', { name: 'O que a Mika lembra' })).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)

  // Somente leitura: nada de erro nem "carregando" preso depois que a tool responde. Sem memória
  // gravada nesta run do E2E, o card cai no estado vazio.
  await expect(settings.getByText('Nenhuma memória registrada ainda.')).toBeVisible({ timeout: 5000 })

  await settings.getByRole('heading', { name: 'O que a Mika lembra' }).scrollIntoViewIfNeeded()
  await settings.screenshot({ path: 'e2e/screenshots/privacy-memory-card.png' })
})
