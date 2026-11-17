import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { ELEVENLABS_TEST_KEY } from '../fixtures/fake-elevenlabs'

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

type ShellWindow = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

async function readConfigPath(page: Page, path: string): Promise<unknown> {
  return page.evaluate(async (p) => {
    const { config } = await (window as unknown as ShellWindow).electronAPI.runtime.getSettingsConfig()
    return p.split('.').reduce<unknown>((node, key) => (node as Record<string, unknown> | undefined)?.[key], config)
  }, path)
}

// A chave salva chega ao renderer MASCARADA; antes desta correção a aba TTS dizia "insira a API key"
// mesmo com ela salva. O dublê só responde à chave real, então ver as vozes prova o caminho inteiro.
test('ElevenLabs: a chave salva lista as vozes, toca a prévia e a escolhida vira a voz da Mika', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await page.evaluate(async (key) => {
    const api = (window as unknown as ShellWindow).electronAPI.runtime
    await api.saveSetting('apiKeys.elevenLabs', key)
    await api.saveSetting('voice.ttsProvider', 'elevenlabs')
  }, ELEVENLABS_TEST_KEY)
  expect(await readConfigPath(page, 'apiKeys.elevenLabs')).toBe('[REDACTED]')

  const settings = await openSettings(page)
  await navigateToTab(settings, 'Fala', /Fala|TTS/)
  const aurora = settings.getByRole('option', { name: /Aurora/ })
  const bento = settings.getByRole('option', { name: /Bento/ })
  await expect(aurora).toBeVisible()
  await expect(bento).toBeVisible()

  const play = settings.getByRole('button', { name: 'Ouvir voz Aurora' })
  await settings.evaluate(() => {
    const original = HTMLMediaElement.prototype.play
    HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
      ;(window as unknown as { __previewSrc?: string }).__previewSrc = this.src
      return original.call(this)
    }
  })
  await play.click()
  await expect
    .poll(() => settings.evaluate(() => (window as unknown as { __previewSrc?: string }).__previewSrc ?? ''))
    .toMatch(/^data:audio\/wav/)

  await bento.click()
  await expect(bento).toHaveAttribute('aria-selected', 'true')
  await expect.poll(() => readConfigPath(settings, 'voice.elevenLabsVoiceId')).toBe('voz-bento')
  await bento.scrollIntoViewIfNeeded()
  await settings.screenshot({ path: artifactPath('voice-settings-elevenlabs-voices.png') })
  writeArtifact('voice-settings-elevenlabs-voices', {
    listed: ['Aurora', 'Bento'],
    previewPlayed: 'Aurora',
    selectedVoiceId: 'voz-bento',
  })
})
