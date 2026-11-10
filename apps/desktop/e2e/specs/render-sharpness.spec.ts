import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'
import { expect, test } from '../fixtures/tauri-app'

type ElectronApi = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

const AVATAR_CANVAS = 'canvas.live2d-container'

// Com GL por software (contêiner, CI) o avatar e o palco 3D da aba Skin disputam a mesma thread:
// reaplicar a densidade leva vários segundos, e a leitura do canvas só volta depois do quadro. O
// valor esperado não muda — só o prazo para ele aparecer.
const SLOW_GL = { timeout: 60_000 }

/** Pixels de backing store por px CSS do canvas do avatar — a nitidez que de fato está na tela. */
const avatarDensity = (page: Page) =>
  page.evaluate((selector) => {
    const canvas = document.querySelector<HTMLCanvasElement>(selector)
    return canvas?.clientWidth ? Number((canvas.width / canvas.clientWidth).toFixed(2)) : 0
  }, AVATAR_CANVAS)

/** O que ficou na tela neste passo: densidade e tamanho real do backing store (o custo de GPU é a área). */
const measure = (page: Page) =>
  page.evaluate((selector) => {
    const canvas = document.querySelector<HTMLCanvasElement>(selector)
    const density = canvas?.clientWidth ? Number((canvas.width / canvas.clientWidth).toFixed(2)) : 0
    return { density, width: canvas?.width ?? 0, height: canvas?.height ?? 0 }
  }, AVATAR_CANVAS)

/** Arrastar a janela para um monitor de outra densidade: o DPR muda e o tamanho em px CSS não. */
async function moveToMonitor(page: Page, deviceScaleFactor: number) {
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 0, height: 0, deviceScaleFactor, mobile: false })
}

async function openSkinTab(page: Page): Promise<Page> {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByRole('navigation').getByRole('button', { name: 'Mika', exact: true }).click()
  await settings.locator('button').filter({ hasText: 'Skin' }).first().click()
  // A aba monta o palco 3D da skin antes das seções: com GL por software isso passa dos 10 s padrão.
  await expect(settings.getByRole('heading', { name: 'Qualidade do render' })).toBeVisible({ timeout: 60_000 })
  return settings
}

test.describe('nitidez do render do avatar', () => {
  test('acompanha a troca de monitor e obedece ao modo escolhido na aba Skin', async ({ app }) => {
    test.setTimeout(360_000)
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await expect(page.locator(AVATAR_CANVAS)).toHaveCount(1, { timeout: 30_000 })

    await expect.poll(() => avatarDensity(page), SLOW_GL).toBe(1)
    const auto1x = await measure(page)

    // Sem ouvir o DPR o canvas continuava em 1x numa tela 2x: borrado até alguém redimensionar a janela.
    await moveToMonitor(page, 2)
    await expect.poll(() => avatarDensity(page), SLOW_GL).toBe(2)
    const auto2x = await measure(page)
    await waitForUiIdle(page)
    await page.screenshot({ path: artifactPath('render-sharpness-avatar-auto-2x-monitor.png') })

    const settings = await openSkinTab(page)
    // `.last()`: o painel inteiro também é uma <section> que contém o título; a seção do render é a de dentro.
    const section = settings
      .locator('section')
      .filter({ has: settings.getByRole('heading', { name: 'Qualidade do render' }) })
      .last()

    await section.getByRole('radio', { name: /Desempenho/ }).click()
    await expect.poll(() => avatarDensity(page), SLOW_GL).toBe(1)
    const performance2x = await measure(page)
    await waitForUiIdle(page)
    await page.screenshot({ path: artifactPath('render-sharpness-avatar-performance.png') })

    // De volta à tela 1x antes do modo nítido: 3x (nítido numa tela 2x) com GL por software não
    // fecha um quadro no tempo do spec. O teto de 3x é do teste de unidade de `render-pixel-ratio`.
    await moveToMonitor(page, 1)
    await section.getByRole('radio', { name: /Nítida/ }).click()
    await expect.poll(() => avatarDensity(page), SLOW_GL).toBe(1.5)
    const sharp1x = await measure(page)
    await waitForUiIdle(page)
    await page.screenshot({ path: artifactPath('render-sharpness-avatar-sharp.png') })

    const saved = await page.evaluate(async () => {
      const { config } = await (window as unknown as ElectronApi).electronAPI.runtime.getSettingsConfig()
      return (config.character as { renderSharpness?: string }).renderSharpness
    })
    expect(saved).toBe('sharp')

    await waitForUiIdle(settings)
    await section.scrollIntoViewIfNeeded()
    await settings.screenshot({ path: artifactPath('render-sharpness-settings-skin.png') })
    writeArtifact('render-sharpness', {
      measured: { auto1x, auto2x, performance2x, sharp1x },
      saved,
    })
    await settings.close()
  })
})
