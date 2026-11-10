import { expect, type Page } from '@playwright/test'

// The dock auto-hides below a clipping shell; keyboard focus reveals it, and unlike a synthetic hover the OS cursor poll cannot undo it.
export async function clickDockButton(page: Page, name: string): Promise<void> {
  const button = page.getByRole('button', { name, exact: true })
  await page.keyboard.press('Shift')
  await button.focus()
  await expect(button).toBeInViewport()
  await button.click()
}
