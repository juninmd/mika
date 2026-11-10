import type { Locator } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'

// O que prova que um switch funciona é o estado dele virar e voltar (e persistir pelo painel), não um
// texto de status que muda a cada revisão de copy.
async function flipAndBack(toggle: Locator): Promise<void> {
  const before = await toggle.getAttribute('aria-checked')
  const flipped = before === 'true' ? 'false' : 'true'
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', flipped)
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', before ?? 'false')
}

test('Person Catalog settings panel renders and toggles work', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill('person-catalog')
  await settings.keyboard.press('Enter')

  await expect(settings.getByText(/Catálogo|Catalogo|person-catalog|Catalogação ativa/i).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)

  const exercised = { enabledToggle: false, autoExtractToggle: false, webEnrichmentToggle: false }
  // O switch do hero leva o nome da aba: sem ele o leitor de tela ouvia um `switch` anônimo.
  const enabledToggle = settings.getByRole('switch', { name: 'Catálogo de Pessoas', exact: true })
  await expect(enabledToggle).toBeVisible()
  await flipAndBack(enabledToggle)
  exercised.enabledToggle = true

  const autoExtractToggle = settings.getByRole('switch', { name: /Extração automática/i })
  await expect(autoExtractToggle).toBeVisible()
  await flipAndBack(autoExtractToggle)
  exercised.autoExtractToggle = true

  const webEnrichmentToggle = settings.getByRole('switch', { name: /Consultar novas pessoas na web/i })
  await expect(webEnrichmentToggle).toBeVisible()
  await flipAndBack(webEnrichmentToggle)
  exercised.webEnrichmentToggle = true
  writeArtifact('person-catalog-settings-controls', exercised)
  await settings.screenshot({ path: artifactPath('person-catalog-settings.png') })
})
