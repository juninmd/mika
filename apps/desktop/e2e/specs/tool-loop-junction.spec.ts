import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('laço de tools: o turno de chat chama, executa e conclui', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  const observed = await page.evaluate(async () => {
    const api = (
      window as unknown as {
        electronAPI: {
          runtime: {
            onToolCall: (cb: (p: { name: string }) => void) => () => void
            onToolResult: (cb: (p: { name: string; success?: boolean }) => void) => () => void
            sendChatMessage: (text: string) => Promise<{ content: string; toolCount: number }>
          }
        }
      }
    ).electronAPI

    const calls: string[] = []
    const results: Array<{ name: string; success?: boolean }> = []
    const unsubCall = api.runtime.onToolCall((p) => calls.push(p.name))
    const unsubResult = api.runtime.onToolResult((p) => results.push({ name: p.name, success: p.success }))
    try {
      const reply = await api.runtime.sendChatMessage('HOMOLOGA_TOOL_LOOP: como está a máquina?')
      return { calls, results, content: reply.content, toolCount: reply.toolCount }
    } finally {
      unsubCall()
      unsubResult()
    }
  })

  expect(observed.calls).toContain('get_system_info')

  expect(observed.results.find((r) => r.name === 'get_system_info')?.success).toBe(true)

  expect(observed.content.length).toBeGreaterThan(0)
  expect(observed.toolCount).toBeGreaterThan(0)
  writeArtifact('tool-loop-junction', observed)
})
