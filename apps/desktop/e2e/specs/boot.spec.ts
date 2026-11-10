import { expect, type TauriShellApp, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'

test('boots the desktop shell with a typed bridge and runtime health @smoke', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await expect
    .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
    .toBe(true)

  const health = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getHealth(),
  )
  expect(health).toEqual(
    expect.objectContaining({
      status: expect.any(String),
      capabilities: expect.objectContaining({ plugins: expect.any(Number) }),
    }),
  )
  writeArtifact('boot-health', { status: health.status, capabilities: health.capabilities })
})

test('opens the settings window and renders its content @smoke', async ({ app }: { app: TauriShellApp }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
    .toBe(true)

  const winPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await winPromise
  await settings.waitForLoadState('domcontentloaded')

  await expect(settings.getByRole('main')).toBeVisible()
  await settings.screenshot({ path: artifactPath('boot-settings-window.png') })
  await settings.close()
})
