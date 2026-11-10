import { expect, type Locator, type Page } from '@playwright/test'
import { clickDockButton } from './dock'

export async function openComposer(page: Page): Promise<Locator> {
  const input = page.locator('textarea[name="prompt"]')
  if (await input.isVisible().catch(() => false)) return input
  await clickDockButton(page, 'Chat (Ctrl+Space)')
  await expect(input).toBeVisible()
  return input
}
