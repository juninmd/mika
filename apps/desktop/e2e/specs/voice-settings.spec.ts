import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'

async function openSettings(page: Page): Promise<Page> {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  return settings
}

async function navigateToTab(settings: Page, query: string, heading: string | RegExp) {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
  await expect(settings.getByRole('heading', { name: heading }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

async function readTtsEnabled(page: Page): Promise<boolean> {
  return page.evaluate(async () => {
    const { config } = await (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getSettingsConfig()
    return (config.voice as { ttsEnabled: boolean }).ttsEnabled
  })
}

test('TTS settings panel renders and toggles work', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  let settings = await openSettings(page)

  await navigateToTab(settings, 'Fala', /Fala|TTS/)
  const original = await readTtsEnabled(settings)
  expect(typeof original).toBe('boolean')
  const toggle = settings.getByRole('switch', { name: 'Mika fala', exact: true })
  await expect(toggle).toHaveAttribute('aria-checked', String(original))

  try {
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', String(!original))
    await expect.poll(() => readTtsEnabled(settings)).toBe(!original)

    // The user's close path: CDP page.close() kills the webview but leaves the Tauri window alive.
    const closed = settings.waitForEvent('close')
    await settings.getByRole('button', { name: 'Fechar', exact: true }).click()
    await closed
    settings = await openSettings(page)
    await navigateToTab(settings, 'Fala', /Fala|TTS/)
    const persistedToggle = settings.getByRole('switch', { name: 'Mika fala', exact: true })
    await expect(persistedToggle).toHaveAttribute('aria-checked', String(!original))
    await expect.poll(() => readTtsEnabled(settings)).toBe(!original)

    await persistedToggle.click()
    await expect(persistedToggle).toHaveAttribute('aria-checked', String(original))
    await expect.poll(() => readTtsEnabled(settings)).toBe(original)
    writeArtifact('voice-settings-tts-toggle', { original, toggled: !original, persistedAfterReopen: !original, restored: original })
  } finally {
    await page.evaluate(async (value) => {
      await (
        window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
      ).electronAPI.runtime.saveSetting('voice.ttsEnabled', value)
    }, original)
  }
})

test('STT settings panel renders and options are visible', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settings = await openSettings(page)

  await navigateToTab(settings, 'Escuta', /Escuta|STT/)
  await expect(settings.getByText(/Provedor|ASR|Escuta|STT/i).first()).toBeVisible()
  await settings.screenshot({ path: artifactPath('voice-settings-stt.png') })
})
