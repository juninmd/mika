import { DEFAULT_MODEL_PATH } from '../../src/shared/avatar-model'
import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

const MODEL_FILE = DEFAULT_MODEL_PATH.split('/').pop() ?? DEFAULT_MODEL_PATH

test.describe('@skin boot da skin selecionada', () => {
  test('o avatar padrão monta e o modelo dele é servido', async ({ app }) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await expect
      .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
      .toBe(true)

    // Ouvir `page.on('response')` depois de a janela já ter subido era corrida perdida: o modelo
    // costumava ser baixado antes de o ouvinte existir, e o teste passava ou falhava conforme o
    // momento em que o app terminava de bootar. Recarregar com o ouvinte no lugar é determinístico.
    const modelRequests: Array<{ url: string; status: number }> = []
    page.on('response', (response) => {
      if (response.url().includes(MODEL_FILE)) {
        modelRequests.push({ url: response.url(), status: response.status() })
      }
    })
    await page.reload()
    await page.waitForLoadState('domcontentloaded')

    // Sem `MIKA_E2E_SKIN`, o app abre no default — o VRM próprio da Mika, único asset de avatar que
    // vai dentro do instalador. O canvas do VRM é o próprio elemento com a classe; o do Live2D é um
    // `<canvas>` dentro dela (esse caminho é coberto por `live2d-arm-probe`, que fixa a skin).
    await expect(page.locator('canvas.live2d-container')).toHaveCount(1, { timeout: 30_000 })

    await expect.poll(() => modelRequests.length, { timeout: 30_000 }).toBeGreaterThan(0)
    for (const request of modelRequests) {
      expect(request.status, `${request.url} respondeu ${request.status}`).toBe(200)
    }
    writeArtifact('skin-boot-model-requests', { modelFile: MODEL_FILE, requests: modelRequests })
  })
})
