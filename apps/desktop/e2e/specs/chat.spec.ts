import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('exposes chat history and clear-history runtime APIs', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const before = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getChatHistory(),
  )
  expect(before.messages).toEqual(expect.any(Array))

  const cleared = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.clearChatHistory(),
  )
  expect(cleared).toEqual(expect.objectContaining({ success: true }))
  writeArtifact('chat-history-api', { messagesBefore: before.messages.length, clearSuccess: cleared.success })
})
