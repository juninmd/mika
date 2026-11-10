import { join } from 'node:path'
import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { expect, test } from '../fixtures/tauri-app'
import type { FakeOllama } from '../fixtures/fake-ollama-server'

type Bridge = Window & { electronAPI: ElectronShellApi }

async function send(page: Page, content: string) {
  const reply = await page.evaluate((text) => (window as unknown as Bridge).electronAPI.runtime.sendChatMessage(text), content)
  expect(reply.id).not.toBe('')
  expect(reply.content.trim()).not.toBe('')
  const history = await page.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.getChatHistory())
  expect(history.messages.find((message) => message.id === reply.id)?.role).toBe('assistant')
}

function outgoingSystem(llm: FakeOllama, userText: string): string {
  const body = llm.requests.map((request) => request.body as { messages?: { role: string; content: string }[] })
    .find((request) => request.messages?.some((message) => message.role === 'user' && message.content === userText))
  expect(body, 'requisição HTTP real do Electron ao provedor de teste').toBeDefined()
  return body?.messages?.filter((message) => message.role === 'system').map((message) => message.content).join('\n') ?? ''
}

test('configurações de persona e instruções chegam ao HTTP e ao trace sem esperar o TTL', async ({ app, llm }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect.poll(() => page.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady)).toBe(true)
  await send(page, 'primeiro turno da homologação')

  const fields = ['visualDescription', 'greetingStyle', 'hobbies', 'nicknamesForUser', 'secretGoals', 'customInstructions']
  for (const field of fields) {
    const saved = await page.evaluate(async (key) => (window as unknown as Bridge).electronAPI.runtime.saveSetting(`character.${key}`, `MARCADOR_${key}`), field)
    expect(saved.success).toBe(true)
  }
  for (const [key, value] of [['llm.systemPrompt', 'DIRETRIZ_TEMPORARIA_731'], ['user.name', 'USUARIO_731'], ['character.responseStyle', 'formal']]) {
    const saved = await page.evaluate(async ({ key, value }) => (window as unknown as Bridge).electronAPI.runtime.saveSetting(key, value), { key: key as string, value })
    expect(saved.success).toBe(true)
  }
  const question = 'segundo turno da homologação'
  await send(page, question)
  const system = outgoingSystem(llm, question)
  for (const field of fields) expect(system).toContain(`MARCADOR_${field}`)
  expect(system).toContain('DIRETRIZ_TEMPORARIA_731')
  expect(system).toContain('USUARIO_731')
  expect(system).toContain('Response style:** formal')
  expect(system).not.toMatch(/\{\{\w+\}\}/)

  const snapshot = await page.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.executeTool('memory_console', { action: 'snapshot' }))
  expect(snapshot.success).toBe(true)
  const trace = (snapshot.data as { promptTrace: { userContent: string; systemPrompt: string } }).promptTrace
  expect(trace.userContent).toBe(question)
  for (const field of fields) expect(trace.systemPrompt).toContain(`MARCADOR_${field}`)
})

test('painel envia o prompt digitado e mostra o retorno do provedor', async ({ app, llm }) => {
  const main = await app.firstWindow()
  await main.waitForLoadState('domcontentloaded')
  const opened = app.waitForEvent('window')
  await main.evaluate(() => (window as unknown as Bridge).electronAPI.shell.openSettings())
  const settings = await opened
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('LLM')
  await settings.keyboard.press('Enter')
  const input = settings.getByPlaceholder('Digite um prompt para testar')
  const additional = settings.getByLabel('Instruções adicionais', { exact: true })
  await additional.fill('DIRETRIZ_PELA_UI_731')
  await additional.press('Tab')
  await expect.poll(() => settings.evaluate(async () => {
    const result = await (window as unknown as Bridge).electronAPI.runtime.getSettingsConfig()
    return (result.config.llm as { systemPrompt?: string }).systemPrompt
  })).toBe('DIRETRIZ_PELA_UI_731')
  const button = settings.getByRole('button', { name: 'Enviar Prompt', exact: true })
  await expect(input).toBeVisible()
  await expect(button).toBeDisabled()
  await input.fill('   ')
  await expect(button).toBeDisabled()
  const question = 'homologacao do painel 731'
  await input.fill(question)
  await button.click()
  await expect(button).toBeEnabled({ timeout: 30000 })
  await expect(settings.getByText(/saquei, tô contigo nessa/)).toBeVisible()
  expect(llm.requests.some((request) => JSON.stringify(request.body).includes(question))).toBe(true)
  await send(main, 'conversa depois de salvar pela UI')
  expect(outgoingSystem(llm, 'conversa depois de salvar pela UI')).toContain('DIRETRIZ_PELA_UI_731')
  await input.scrollIntoViewIfNeeded()
  await settings.screenshot({ path: join(__dirname, '../screenshots/prompts-panel.png') })
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('Memória')
  await settings.keyboard.press('Enter')
  await settings.getByText('Último Prompt', { exact: true }).click()
  await settings.getByRole('button', { name: 'Ver', exact: true }).click()
  const trace = settings.locator('pre').filter({ hasText: 'DIRETRIZ_PELA_UI_731' })
  await expect(trace).toBeVisible()
  await trace.scrollIntoViewIfNeeded()
  await settings.screenshot({ path: join(__dirname, '../screenshots/prompts-trace.png') })
})
