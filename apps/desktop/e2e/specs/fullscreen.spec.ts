import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { expect, test } from '../fixtures/tauri-app'
import { waitForUiIdle } from '../fixtures/settled'

type Bridge = Window & { electronAPI: ElectronShellApi }

// A janela nativa de verdade é consultada pelo canal `window-get-fullscreen` (o backend pergunta ao
// Tauri). No host HTTP do E2E (Linux/macOS) a UI roda num Chromium à parte, então o PNG mostra o
// renderer — o estado da janela nativa é provado pelo canal e gravado no JSON, não pela imagem.
const nativeFullscreen = (page: Page) =>
  page.evaluate(() => (window as unknown as Bridge).electronAPI.shell.window?.isFullscreen?.())

const savedPreference = (page: Page) =>
  page.evaluate(async () => {
    const { config } = await (window as unknown as Bridge).electronAPI.runtime.getSettingsConfig()
    return (config as { app?: { fullscreen?: boolean } }).app?.fullscreen
  })

test('tela cheia: interruptor de Configurações e F11 comandam a janela real e persistem', async ({ app }) => {
  const main = await app.firstWindow()
  await main.waitForLoadState('domcontentloaded')
  await waitForUiIdle(main)

  expect(await nativeFullscreen(main), 'o avatar nasce em janela').toBe(false)
  await main.screenshot({ path: artifactPath('fullscreen-1-windowed.png') })

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(main, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  const toggle = settings.getByRole('switch', { name: /Tela cheia/i })
  await toggle.scrollIntoViewIfNeeded()
  await expect(toggle).toBeVisible()
  await expect(settings.getByRole('status').filter({ hasText: /Em janela/ })).toBeVisible()

  await toggle.click()
  await expect.poll(() => nativeFullscreen(main), { message: 'o interruptor tem de ligar a tela cheia da janela' }).toBe(true)
  await expect.poll(() => savedPreference(main)).toBe(true)
  await expect(settings.getByRole('status').filter({ hasText: /Tela cheia/ })).toBeVisible()
  await expect(toggle).toBeChecked()
  await settings.screenshot({ path: artifactPath('fullscreen-2-settings-on.png') })

  // F11 com a janela do avatar em foco desliga; o interruptor de Configurações acompanha pelo evento.
  await main.keyboard.press('F11')
  await expect.poll(() => nativeFullscreen(main), { message: 'F11 tem de sair da tela cheia' }).toBe(false)
  await expect.poll(() => savedPreference(main)).toBe(false)
  await expect(settings.getByRole('status').filter({ hasText: /Em janela/ })).toBeVisible()
  await expect(toggle).not.toBeChecked()
  await settings.screenshot({ path: artifactPath('fullscreen-3-settings-off-after-f11.png') })

  await main.keyboard.press('F11')
  await expect.poll(() => nativeFullscreen(main)).toBe(true)
  await main.screenshot({ path: artifactPath('fullscreen-4-main-fullscreen.png') })

  await toggle.click()
  await expect.poll(() => nativeFullscreen(main)).toBe(false)

  writeArtifact('fullscreen', {
    bootNative: false,
    afterSettingsToggleOn: { native: true, savedPreference: true },
    afterF11Off: { native: false, savedPreference: false },
    afterF11On: { native: true },
    afterSettingsToggleOff: { native: false },
  })
})
