import { join } from 'node:path'
import { expect, test } from '../fixtures/tauri-app'

// Relativo ao spec, não ao `cwd`: rodar do raiz do repo escrevia o print num `e2e/screenshots/`
// novo lá fora, e o print que homologa a tela é o que ninguém abriu.
const SCREENSHOTS_DIR = join(__dirname, '..', 'screenshots')

/** Prova de que o usuário vê a Mika ouvindo — gate verde não prova tela. */
test('o HUD mostra a onda sonora enquanto o microfone está ligado', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await page.evaluate(async () => {
    const api = (window as unknown as { electronAPI: { runtime: { saveSetting: (p: string, v: unknown) => unknown } } })
      .electronAPI
    await api.runtime.saveSetting('voice.sttEnabled', true)
    await api.runtime.saveSetting('voice.inputMode', 'continuous')
  })

  const waveform = page.getByTestId('voice-waveform')
  await expect(waveform).toBeVisible({ timeout: 20_000 })

  // A onda tem de ser legível por leitor de tela, não só desenho.
  await expect(waveform).toHaveAttribute('aria-label', /.+/)

  // Simula áudio no bus. O app também lê o microfone real e publica `hearing: false` por cima; um único
  // `publish` só vence por sorte de tempo. Republica a cada tentativa até o HUD refletir.
  await expect
    .poll(
      async () => {
        await page.evaluate(() => {
          const bus = (
            window as unknown as { __mikaMicBus?: { publish: (s: { level: number; hearing: boolean }) => void } }
          ).__mikaMicBus
          bus?.publish({ level: 0.35, hearing: true })
        })
        return waveform.getAttribute('data-hearing')
      },
      { timeout: 10_000 },
    )
    .toBe('true')

  await page.screenshot({ path: join(SCREENSHOTS_DIR, 'voice-waveform-hearing.png') })
})
