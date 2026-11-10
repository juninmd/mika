import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './fixtures/homolog-app'

const PATHS: Array<{ path: string; value: unknown; painel: string }> = [
  { path: 'llm.provider', value: 'ollama', painel: 'LLMConnectionPanel' },
  { path: 'llm.model', value: 'qwen2.5:7b', painel: 'LLMConnectionPanel' },
  { path: 'llm.ollamaModel', value: 'qwen2.5:7b', painel: 'LLMConnectionPanel' },
  { path: 'llm.googleModel', value: 'gemini-2.0-flash', painel: 'LLMConnectionPanel' },
  { path: 'llm.openrouterModel', value: 'openai/gpt-oss-20b:free', painel: 'LLMConnectionPanel' },
  { path: 'llm.openaiCompatibleModel', value: 'local-model', painel: 'LLMConnectionPanel' },
  { path: 'llm.unslothModel', value: 'local-model', painel: 'LLMConnectionPanel' },
  { path: 'llm.systemPrompt', value: 'teste', painel: 'LLMConnectionPanel' },
  { path: 'llm.embeddingProvider', value: 'ollama', painel: 'LLMConnectionPanel' },
  { path: 'llm.embeddingModel', value: 'nomic-embed-text', painel: 'LLMConnectionPanel' },
]

test('settings: os caminhos que os painéis gravam são aceitos pelo IPC', async ({ window }) => {
  const results = await window.evaluate(async (paths) => {
    const runtime = (
      window as unknown as {
        electronAPI: { runtime: { saveSetting: (p: string, v: unknown) => Promise<unknown> } }
      }
    ).electronAPI.runtime
    const out: Array<{ path: string; ok: boolean; error?: string }> = []
    for (const item of paths) {
      try {
        await runtime.saveSetting(item.path, item.value)
        out.push({ path: item.path, ok: true })
      } catch (error) {
        out.push({ path: item.path, ok: false, error: error instanceof Error ? error.message : String(error) })
      }
    }
    return out
  }, PATHS)

  const rejeitados = results.filter((r) => !r.ok)
  const outDir = join(__dirname, 'report')
  mkdirSync(outDir, { recursive: true })
  writeFileSync(join(outDir, 'settings-paths.json'), JSON.stringify(results, null, 2))

  for (const item of rejeitados) console.log(`RECUSADO ${item.path}: ${item.error}`)
  console.log(`settings: ${results.length - rejeitados.length}/${results.length} caminhos aceitos`)

  expect(rejeitados.map((r) => r.path)).toEqual([])
})
