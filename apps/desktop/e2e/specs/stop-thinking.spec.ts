import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'
import { expect, test } from '../fixtures/tauri-app'

const THINKING = /PENSANDO/i

// A provider that accepts the request and never streams left her "pensando" with no way out: the dock's
// stop only existed while TTS played. The stop must show while thinking, end it, and free the next turn.
test('parar durante "pensando" cancela o turno travado e libera o próximo', async ({ app, llm }) => {
  test.setTimeout(120_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect(page.getByTestId('voice-waveform')).toBeVisible({ timeout: 20_000 })
  await expect(page.getByText(THINKING)).toHaveCount(0, { timeout: 15_000 })
  const stop = page.getByRole('button', { name: 'Cancelar resposta', exact: true })
  await expect(stop, 'sem nada em voo não há botão de parar').toHaveCount(0)

  // Stands in for the stuck provider: longer than the whole assertion window below.
  llm.setChatDelay(90_000)
  await clickDockButton(page, 'Chat (Ctrl+Space)')
  await page.locator('textarea[name="prompt"]').fill('oi mika, tudo bem?')
  await page.keyboard.press('Enter')
  await expect(page.getByText(THINKING).first()).toBeVisible()
  await expect(stop).toHaveCount(1)
  await page.keyboard.press('Shift')
  await stop.focus()
  await page.screenshot({ path: artifactPath('stop-thinking-before.png') })

  await clickDockButton(page, 'Cancelar resposta')
  await expect(page.getByText(THINKING), 'parar tira do "pensando" sem esperar o provedor').toHaveCount(0, {
    timeout: 3_000,
  })
  await expect(stop).toHaveCount(0)
  await waitForUiIdle(page)
  await page.screenshot({ path: artifactPath('stop-thinking-after.png') })

  // The cancelled turn must not hold the send mutex: the next message goes out and gets answered.
  llm.setChatDelay(0)
  await clickDockButton(page, 'Chat (Ctrl+Space)')
  await page.locator('textarea[name="prompt"]').fill('e agora, me responde?')
  await page.keyboard.press('Enter')
  await expect
    .poll(() => llm.requests.some((r) => JSON.stringify(r.body).includes('e agora, me responde?')), {
      message: 'o próximo turno saiu para o provedor',
    })
    .toBe(true)
  await expect(page.getByText(THINKING)).toHaveCount(0, { timeout: 15_000 })

  writeArtifact('stop-thinking', { stopShownWhileThinking: true, thinkingClearedAfterStop: true, nextTurnSent: true })
})
