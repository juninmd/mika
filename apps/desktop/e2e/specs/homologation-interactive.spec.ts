import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { artifactPath } from '../fixtures/artifact'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'

async function navigateToSettingsTab(settings: Page, query: string): Promise<void> {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
}

async function expectPanelVisible(settings: Page, panelText: string | RegExp): Promise<void> {
  await expect(settings.getByText(panelText).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

test('drives desktop UI, settings, and visual tool execution', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await expect(settings.getByLabel('Abrir paleta de comandos')).toBeVisible()

  await navigateToSettingsTab(settings, 'person-catalog')
  await expectPanelVisible(settings, /Catálogo|person-catalog/i)
  await navigateToSettingsTab(settings, 'vision')
  await expectPanelVisible(settings, /Visão|capture|qualidade/i)
  await navigateToSettingsTab(settings, 'overview')
  await expectPanelVisible(settings, /Resumo|Overview|Módulos/i)

  await enableShellChrome(page)
  await page.keyboard.press('Control+k')
  await expect(page.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible()

  await page.getByPlaceholder('Buscar comandos e ferramentas…').fill('memory console')
  await expect(page.getByText('memory console', { exact: true })).toBeVisible()
  await page.keyboard.press('Enter')

  await expect(page.getByPlaceholder('Buscar comandos e ferramentas…')).toHaveCount(0)
  await expect(page.getByText(/Falha ao executar ferramenta/)).toHaveCount(0)
  await page.screenshot({ path: artifactPath('homologation-interactive-final.png') })
})
