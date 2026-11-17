import { cpSync, existsSync, mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test as base } from '../fixtures/tauri-app'

// Relative to the spec, like every other artifact here.
const SCREENSHOTS_DIR = join(__dirname, '..', 'screenshots')
const SPEECH_WAV = join(__dirname, '..', 'fixtures', 'voice', 'fala-usuario-pt-br.wav')
// Whisper weights are never downloaded by the suite: point this at a folder holding
// `encoder_model_quantized.onnx`, `decoder_model_merged_quantized.onnx` and `tokenizer.json`
// of onnx-community/whisper-base (revision pinned in inference/whisper.rs).
const WHISPER_DIR = process.env.MIKA_E2E_WHISPER_DIR

const test = base.extend({
  profile: async ({ profile }, use) => {
    if (WHISPER_DIR) {
      const dest = join(profile.userDataDir, 'whisper-cache', 'whisper-base')
      mkdirSync(dest, { recursive: true })
      cpSync(WHISPER_DIR, dest, { recursive: true })
    }
    await use(profile)
  },
})

test.beforeEach(() => {
  process.env.MIKA_E2E_FAKE_AUDIO = SPEECH_WAV
})
test.afterEach(() => {
  delete process.env.MIKA_E2E_FAKE_AUDIO
})

test('o chat aberto mostra a fala aparecendo aos poucos enquanto o usuário fala', async ({ app }) => {
  test.skip(!WHISPER_DIR || !existsSync(join(WHISPER_DIR, 'tokenizer.json')), 'MIKA_E2E_WHISPER_DIR não aponta para os pesos')
  test.setTimeout(180_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await page.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 200, clientY: 200 })
  await page.getByRole('menu').getByRole('menuitem', { name: /^Histórico/i }).click()
  const history = page.locator('[role="dialog"][aria-labelledby="chat-history-title"]')
  await expect(history).toBeVisible()

  await page.evaluate(async () => {
    const api = (window as unknown as { electronAPI: { runtime: { saveSetting: (p: string, v: unknown) => unknown } } })
      .electronAPI
    await api.runtime.saveSetting('voice.asrProvider', 'local-whisper')
    // The looped fixture pauses ~1 s between sentences: below the 1200 ms default, a turn would never end.
    await api.runtime.saveSetting('voice.vadSilenceMs', 600)
    await api.runtime.saveSetting('voice.sttEnabled', true)
    await api.runtime.saveSetting('voice.inputMode', 'continuous')
  })

  // Every distinct state the bubble goes through, so the artifact shows the text growing.
  await page.evaluate(() => {
    const seen: string[] = []
    ;(window as unknown as { __liveSeen: string[] }).__liveSeen = seen
    new MutationObserver(() => {
      const text = document.querySelector('[data-testid="live-transcript"]')?.textContent ?? ''
      if (text && seen.at(-1) !== text) seen.push(text)
    }).observe(document.body, { subtree: true, childList: true, characterData: true })
  })

  const bubble = history.getByTestId('live-transcript')
  await expect(bubble).toBeVisible({ timeout: 60_000 })
  // Past the "Ouvindo você…" label: a partial transcript has arrived before the turn was sent.
  await expect.poll(async () => ((await bubble.textContent()) ?? '').length, { timeout: 60_000 }).toBeGreaterThan(20)
  mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  await page.screenshot({ path: join(SCREENSHOTS_DIR, 'live-transcript-partial.png') })

  // The partial hands over to the real voice turn in the log.
  const spoken = history.locator('article').filter({ has: page.getByRole('img', { name: /voz alta/i }) })
  await expect(spoken.first()).toBeVisible({ timeout: 60_000 })
  await page.screenshot({ path: join(SCREENSHOTS_DIR, 'live-transcript-final.png') })

  const seen = await page.evaluate(() => (window as unknown as { __liveSeen: string[] }).__liveSeen)
  writeFileSync(
    join(SCREENSHOTS_DIR, 'live-transcript.json'),
    JSON.stringify({ partialStates: seen, finalTurn: await spoken.first().textContent() }, null, 2),
  )
  expect(seen.length).toBeGreaterThan(1)
})
