import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { waitForUiIdle } from '../fixtures/settled'
import { expect, test } from '../fixtures/tauri-app'

type Api = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

const VERSION = '99.0.0'

const assistantLines = (page: Page) =>
  page.evaluate(async () => {
    const { messages } = await (window as unknown as Api).electronAPI.runtime.getChatHistory()
    return messages.filter((m) => m.role === 'assistant').map((m) => m.content)
  })

test.use({ updateVersion: VERSION })

// Real path: the boot check hits the local `latest.json`, the backend asks the LLM double for her
// line (prompt `update/new-version-speech.md`), and the card asks before anything is downloaded.
test('versão nova: Mika avisa, pergunta, e "Depois" não instala nada', async ({ app, llm, updateFeed }) => {
  test.setTimeout(120_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const prompt = page.getByTestId('update-prompt')
  await expect(prompt).toBeVisible({ timeout: 60_000 })
  await expect(prompt).toContainText(`A versão ${VERSION} da Mika saiu`)
  await expect(prompt.getByRole('button', { name: 'Atualizar agora' })).toBeEnabled()

  const askedLlm = () => llm.requests.some((r) => JSON.stringify(r.body).includes(`version ${VERSION}`))
  await expect.poll(askedLlm, { timeout: 30_000 }).toBe(true)
  await expect.poll(async () => (await assistantLines(page)).length, { timeout: 30_000 }).toBeGreaterThan(0)

  await waitForUiIdle(page)
  await page.screenshot({ path: artifactPath('update-announce-prompt.png') })

  await prompt.getByRole('button', { name: 'Depois' }).click()
  await expect(prompt).toBeHidden()
  const status = await page.evaluate(() => (window as unknown as Api).electronAPI.runtime.getUpdateStatus())
  // "Depois" leaves the update known but untouched: nothing downloaded, nothing installed.
  expect(status).toEqual(expect.objectContaining({ available: true, version: VERSION }))
  expect(status.progress).toBeUndefined()

  writeArtifact('update-announce', {
    version: VERSION,
    feedHits: updateFeed?.hits() ?? 0,
    spokenLines: (await assistantLines(page)).length,
    statusAfterLater: status,
  })
})
