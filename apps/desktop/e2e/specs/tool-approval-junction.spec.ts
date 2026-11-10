import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

test('aprovação: tool de alto risco pedida pelo modelo pára no gate', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  const observed = await page.evaluate(async () => {
    const api = (
      window as unknown as {
        electronAPI: {
          runtime: {
            onToolApprovalRequest: (cb: (p: { toolCallId: string; toolName: string }) => void) => () => void
            sendToolApprovalResponse: (id: string, approved: boolean, scope?: string) => void
            onToolResult: (cb: (p: { name: string; success?: boolean }) => void) => () => void
            sendChatMessage: (text: string) => Promise<{ content: string }>
          }
        }
      }
    ).electronAPI

    const asked: string[] = []
    const results: Array<{ name: string; success?: boolean }> = []

    // `invoke` response and `webContents.send` events race with no guaranteed order; wait for the event itself, with a timeout.
    let seenClearHistory!: () => void
    const clearHistoryArrived = new Promise<void>((resolve) => {
      seenClearHistory = resolve
    })

    const unsubAsk = api.runtime.onToolApprovalRequest((p) => {
      asked.push(p.toolName)
      api.runtime.sendToolApprovalResponse(p.toolCallId, false, 'once')
    })
    const unsubResult = api.runtime.onToolResult((p) => {
      results.push({ name: p.name, success: p.success })
      if (p.name === 'clear_history') seenClearHistory()
    })
    try {
      const reply = await api.runtime.sendChatMessage('HOMOLOGA_APROVACAO: apaga meu histórico')
      await Promise.race([clearHistoryArrived, new Promise((resolve) => setTimeout(resolve, 5000))])
      return { asked, results, content: reply.content }
    } finally {
      unsubAsk()
      unsubResult()
    }
  })

  expect(observed.asked).toContain('clear_history')

  expect(observed.results.find((r) => r.name === 'clear_history')?.success).toBe(false)

  expect(observed.content.length).toBeGreaterThan(0)
  writeArtifact('tool-approval-junction', observed)
})
