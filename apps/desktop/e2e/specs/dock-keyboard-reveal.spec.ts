import { expect, test } from '../fixtures/tauri-app'
import { artifactPath } from '../fixtures/artifact'

// The shell clips instead of scrolling, so a hidden dock must come back on its own when the keyboard lands on it.
test('keyboard focus reveals the auto-hidden dock', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  const chat = page.getByRole('button', { name: 'Chat (Ctrl+Space)' })
  await page.mouse.move(4, 4)
  await expect(chat).not.toBeInViewport({ timeout: 10_000 })

  await page.keyboard.press('Shift')
  await chat.focus()
  await expect(chat).toBeInViewport()
  await page.screenshot({ path: artifactPath('dock-keyboard-revealed.png') })
  expect(await page.evaluate(() => [...document.querySelectorAll('div')].some((d) => d.scrollTop > 0))).toBe(false)

  await chat.click()
  await page.keyboard.press('Escape')
  await page.mouse.move(4, 4)
  await expect(chat, 'a mouse click must not pin the dock open').not.toBeInViewport({ timeout: 10_000 })
})
