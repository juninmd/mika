import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test.describe('match_effect — efeitos visuais reais', () => {
  test('comando acende o canvas do overlay; repetir desliga', async ({ app }) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    await expect
      .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
      .toBe(true)

    await page.waitForSelector('#root > div', { timeout: 30_000 })

    const effectCanvas = page.locator('canvas.fixed.inset-0.pointer-events-none')
    await expect(effectCanvas).toHaveCount(0)

    const runEffect = (text: string) =>
      page.evaluate((cmd) => {
        const api = (
          window as unknown as {
            electronAPI: { runtime: { executeTool: (n: string, a: { text: string }) => Promise<unknown> } }
          }
        ).electronAPI
        return api.runtime.executeTool('match_effect', { text: cmd })
      }, text)

    const result = await runEffect('/matrix')
    expect(result).toEqual(
      expect.objectContaining({
        success: true,
        data: expect.objectContaining({ effect: 'matrix', triggered: true }),
      }),
    )
    await expect(effectCanvas).toHaveCount(1)

    await runEffect('/matrix')
    await expect(effectCanvas).toHaveCount(0)
    writeArtifact('match-effect-toggle', { firstRun: result, canvasAfterFirst: 1, canvasAfterRepeat: 0 })
  })

  test('texto que não casa com gatilho nenhum não acende nada', async ({ app }) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('#root > div', { timeout: 30_000 })

    const result = await page.evaluate(() => {
      const api = (
        window as unknown as {
          electronAPI: { runtime: { executeTool: (n: string, a: { text: string }) => Promise<unknown> } }
        }
      ).electronAPI
      return api.runtime.executeTool('match_effect', { text: 'bom jogo' })
    })
    expect(result).toEqual(expect.objectContaining({ success: true, data: { triggered: false } }))

    const effectCanvas = page.locator('canvas.fixed.inset-0.pointer-events-none')
    await expect(effectCanvas).toHaveCount(0)
    writeArtifact('match-effect-no-trigger', { result, canvasCount: 0 })
  })
})
