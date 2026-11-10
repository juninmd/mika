import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('tool catalog entries expose stable metadata', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const { tools } = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getTools(),
  )
  expect(tools.length).toBeGreaterThan(0)
  expect(tools[0]).toEqual(
    expect.objectContaining({
      name: expect.any(String),
      description: expect.any(String),
    }),
  )
  writeArtifact('tools-catalog-metadata', { count: tools.length, first: tools[0] })
})
