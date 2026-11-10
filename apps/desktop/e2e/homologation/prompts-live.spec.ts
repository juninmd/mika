import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { expect, test } from './fixtures/homolog-app'

type Bridge = Window & { electronAPI: ElectronShellApi }

test('persona configurada participa de uma conversa com provedor real @external', async ({ window }) => {
  await expect.poll(() => window.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady)).toBe(true)
  const saved = await window.evaluate(async () => {
    const api = (window as unknown as Bridge).electronAPI.runtime
    return Promise.all([
      api.saveSetting('character.name', 'Haru'),
      api.saveSetting('character.customInstructions', 'Quando perguntarem seu nome, identifique-se como Haru.'),
      api.saveSetting('llm.systemPrompt', 'Responda em português em no máximo duas frases.'),
    ])
  })
  expect(saved.every((result) => result.success)).toBe(true)
  const started = Date.now()
  const reply = await window.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.sendChatMessage('Qual é o seu nome?'))
  const snapshot = await window.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.executeTool('memory_console', { action: 'snapshot' }))
  const report = join(__dirname, 'report')
  mkdirSync(report, { recursive: true })
  writeFileSync(join(report, 'prompts-live.json'), JSON.stringify({ date: new Date().toISOString(), ms: Date.now() - started, reply, snapshot }, null, 2))
  expect(reply.id).not.toBe('')
  expect(reply.content).toMatch(/Haru/i)
  const history = await window.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.getChatHistory())
  expect(history.messages.find((message) => message.id === reply.id)?.role).toBe('assistant')
  expect(snapshot.success).toBe(true)
  const trace = (snapshot.data as { promptTrace: { systemPrompt: string } }).promptTrace
  expect(trace.systemPrompt).toContain('identifique-se como Haru')
  expect(trace.systemPrompt).toContain('Responda em português em no máximo duas frases.')
})
