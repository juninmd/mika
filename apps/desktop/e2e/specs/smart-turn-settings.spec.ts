import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'

type ShellWindow = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

const TOGGLE_LABEL = 'Fim de turno inteligente (Smart Turn)'
const CONFIG_PATH = 'voice.smartTurnEnabled'

async function openSettings(page: Page): Promise<Page> {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  return settings
}

async function navigateToListening(settings: Page) {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill('Escuta')
  await settings.keyboard.press('Enter')
  await expect(settings.getByRole('heading', { name: /Escuta|STT/ }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

async function readConfigPath(page: Page, path: string): Promise<unknown> {
  return page.evaluate(async (p) => {
    const { config } = await (window as unknown as ShellWindow).electronAPI.runtime.getSettingsConfig()
    return p.split('.').reduce<unknown>((node, key) => (node as Record<string, unknown> | undefined)?.[key], config)
  }, path)
}

async function restoreDefault(page: Page) {
  await page.evaluate(async (path) => {
    await (window as unknown as ShellWindow).electronAPI.runtime.saveSetting(path, false)
  }, CONFIG_PATH)
}

test('Smart Turn toggle is opt-in and persists voice.smartTurnEnabled', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settings = await openSettings(page)
  await navigateToListening(settings)

  const toggle = settings.getByRole('switch', { name: TOGGLE_LABEL, exact: true })
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  expect(await readConfigPath(settings, CONFIG_PATH)).toBe(false)

  try {
    await toggle.scrollIntoViewIfNeeded()
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true')
    await expect.poll(() => readConfigPath(settings, CONFIG_PATH)).toBe(true)

    // O card de calibração inteiro, com o toggle ligado — o artefato visual da mudança.
    const card = toggle.locator('xpath=ancestor::div[contains(@class,"bg-glass")][1]')
    await card.scrollIntoViewIfNeeded()
    await card.screenshot({ path: artifactPath('smart-turn-settings.png') })

    writeArtifact('smart-turn-settings', {
      toggleLabel: TOGGLE_LABEL,
      defaultEnabled: false,
      enabledInUi: await toggle.getAttribute('aria-checked'),
      configPath: CONFIG_PATH,
      persistedConfig: await readConfigPath(settings, CONFIG_PATH),
    })
  } finally {
    await restoreDefault(settings)
  }
})
