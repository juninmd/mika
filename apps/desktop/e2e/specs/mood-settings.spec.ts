import type { Locator } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'
import { artifactPath, writeArtifact } from '../fixtures/artifact'

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

test('Mood settings panel renders and toggles work', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill('mood')
  await settings.keyboard.press('Enter')

  await expect(settings.getByText(/Sistema de Humor|Humor e Afinidade|Decaimento de Energia/i).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)

  const exercised = { enabledToggle: false, decaySlider: false }
  const enabledToggle = settings.getByRole('switch', { name: /Sistema de Humor ativo/i })
  await expect(enabledToggle).toBeVisible()
  exercised.enabledToggle = true
  await flipAndBack(enabledToggle)

  const decaySlider = settings.getByRole('slider', { name: /Taxa de decaimento de energia/i })
  await expect(decaySlider).toBeVisible()
  exercised.decaySlider = true
  const before = await decaySlider.getAttribute('aria-valuetext')
  await decaySlider.focus()
  await decaySlider.press('ArrowRight')

  // No próprio slider: o painel também mostra a taxa num StatTile, e texto solto casa os dois.
  await expect(decaySlider).toHaveAttribute('aria-valuetext', /%\//)
  await expect(decaySlider).not.toHaveAttribute('aria-valuetext', before ?? '')
  writeArtifact('mood-settings-controls', exercised)
  await settings.screenshot({ path: artifactPath('mood-settings.png') })
})
