import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './fixtures/homolog-app'

test('catálogo: registra o que o app expõe depois do boot', async ({ window }) => {
  const catalog = await window.evaluate(async () => {
    const runtime = (
      window as unknown as {
        electronAPI: {
          runtime: {
            getTools: () => Promise<{ tools: unknown[] }>
            getHealth: () => Promise<unknown>
          }
        }
      }
    ).electronAPI.runtime
    return { tools: (await runtime.getTools()).tools, health: await runtime.getHealth() }
  })

  const outDir = join(__dirname, 'report')
  mkdirSync(outDir, { recursive: true })
  writeFileSync(join(outDir, 'catalog.json'), JSON.stringify(catalog, null, 2))

  const tools = catalog.tools as Array<{ name: string; pluginId: string }>
  const byPlugin = new Map<string, string[]>()
  for (const t of tools) byPlugin.set(t.pluginId, [...(byPlugin.get(t.pluginId) ?? []), t.name])
  for (const [pluginId, names] of [...byPlugin].sort()) {
    console.log(`${pluginId}: ${names.sort().join(', ')}`)
  }
  console.log('health:', JSON.stringify(catalog.health))

  expect(tools.length).toBeGreaterThan(0)
})
