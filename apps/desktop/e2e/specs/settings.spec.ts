import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('loads settings declarations and can persist a simple setting', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const declarations = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getSettingsDeclarations(),
  )
  expect(declarations.declarations.length).toBeGreaterThan(0)

  const saveResult = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.saveSetting('llm.provider', 'ollama'),
  )
  expect(saveResult).toEqual(expect.objectContaining({ success: true }))

  const config = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getSettingsConfig(),
  )
  expect(config.config).toEqual(expect.objectContaining({ llm: expect.objectContaining({ provider: 'ollama' }) }))
  writeArtifact('settings-save-roundtrip', {
    declarationCount: declarations.declarations.length,
    saveSuccess: saveResult.success,
    persistedLlmProvider: (config.config as { llm?: { provider?: string } }).llm?.provider,
  })
})
