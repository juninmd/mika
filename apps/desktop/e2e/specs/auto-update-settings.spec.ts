import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'
import { expect, test } from '../fixtures/tauri-app'

type ElectronApi = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

const savedMode = (page: Page) =>
  page.evaluate(async () => {
    const { config } = await (window as unknown as ElectronApi).electronAPI.runtime.getSettingsConfig()
    return (config.app as { autoUpdate?: string }).autoUpdate
  })

test.describe('atualização automática', () => {
  test('nasce em "instalar sozinha" e o modo escolhido em Atualizações é o que o backend lê', async ({ app }) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await expect
      .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
      .toBe(true)
    expect(await savedMode(page)).toBe('install')

    const settingsPromise = page.context().waitForEvent('page')
    await clickDockButton(page, 'Configurações')
    const settings = await settingsPromise
    await settings.waitForLoadState('domcontentloaded')
    await settings.getByRole('navigation').getByRole('button', { name: 'Sistema', exact: true }).click()
    await settings.locator('button').filter({ hasText: 'Atualizações' }).first().click()
    await expect(settings.getByRole('heading', { name: 'Atualização automática' })).toBeVisible()

    const modes = settings.getByRole('radiogroup', { name: 'Como atualizar' })
    await expect(modes.getByRole('radio', { name: /Instalar sozinha/ })).toHaveAttribute('aria-checked', 'true')

    await modes.getByRole('radio', { name: /Só avisar/ }).click()
    await expect(modes.getByRole('radio', { name: /Só avisar/ })).toHaveAttribute('aria-checked', 'true')
    await expect.poll(() => savedMode(page)).toBe('notify')

    await waitForUiIdle(settings)
    await modes.scrollIntoViewIfNeeded()
    await settings.screenshot({ path: artifactPath('auto-update-settings.png') })
    writeArtifact('auto-update-settings', { defaultMode: 'install', savedAfterChoosingNotify: await savedMode(page) })
    await settings.close()
  })
})
