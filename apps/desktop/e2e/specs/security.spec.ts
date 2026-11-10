import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('renderer has bridge access without direct Node primitives', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const runtime = await page.evaluate(() => ({
    hasBridge: Boolean((window as unknown as { electronAPI?: unknown }).electronAPI),
    hasRequire: typeof (globalThis as unknown as { require?: unknown }).require !== 'undefined',
    hasNodeProcess:
      typeof (globalThis as unknown as { process?: { versions?: { node?: string } } }).process?.versions?.node ===
      'string',
  }))

  expect(runtime).toEqual({
    hasBridge: true,
    hasRequire: false,
    hasNodeProcess: false,
  })
  writeArtifact('security-renderer-surface', runtime)
})
