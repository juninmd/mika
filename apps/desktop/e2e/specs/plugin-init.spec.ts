import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('exposes registered tools and plugin UI catalog after init', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await expect
    .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
    .toBe(true)

  // Plugins finish registering asynchronously after the window becomes ready to evaluate (same race
  // boot.spec.ts already polls around for `electronAPI` itself) — a one-shot read here raced ahead
  // of `mika_desktop_lib` finishing plugin init and observed `capabilities.plugins === 0`.
  const getHealth = () =>
    page.evaluate(() =>
      (
        window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
      ).electronAPI.runtime.getHealth(),
    )
  await expect.poll(async () => (await getHealth()).capabilities.plugins, { timeout: 30_000 }).toBeGreaterThanOrEqual(1)
  const health = await getHealth()
  if (health.status !== 'ok') {
    console.error('Health degraded details:', JSON.stringify(health, null, 2))
  }
  expect(health.status).toBe('ok')

  const tools = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getTools(),
  )
  expect(tools.tools.length).toBeGreaterThanOrEqual(1)
  for (const tool of tools.tools) {
    expect(tool.name).toEqual(expect.any(String))
    expect(tool.description).toEqual(expect.any(String))
  }

  const pluginUi = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getPluginUi(),
  )
  expect(pluginUi).toEqual(expect.any(Object))
  writeArtifact('plugin-init-catalog', {
    healthStatus: health.status,
    plugins: health.capabilities.plugins,
    tools: tools.tools.map((tool) => tool.name).sort(),
    pluginUiKeys: Object.keys(pluginUi).sort(),
  })
})

test('plugin RPC bridge validates requests and returns proper responses', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const result = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.shell.pluginRpc?.({
      pluginId: 'mika.test',
      method: 'ping',
    }),
  )

  expect(result).toBeDefined()
  expect(typeof result).toBe('object')
  writeArtifact('plugin-rpc-response', { result })
})
