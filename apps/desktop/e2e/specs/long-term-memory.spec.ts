import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { writeArtifact } from '../fixtures/artifact'
import type { FakeOllama } from '../fixtures/fake-ollama-server'
import { expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }

// Memória de longo prazo de ponta a ponta, pelo caminho de produção: o turno de chat dispara o
// pós-turno (`post_turn::facts`), que pede a extração ao LLM (o dublê responde no contrato de
// `fact-extraction.md`), grava no grafo social e — no turno SEGUINTE — `facts_block` injeta o fato
// no prompt de sistema que sai por HTTP. Um turno sem fato confirmado não pode gravar nada.

async function send(page: Page, content: string) {
  const reply = await page.evaluate((text) => (window as unknown as Bridge).electronAPI.runtime.sendChatMessage(text), content)
  expect(reply.content.trim()).not.toBe('')
}

async function knownFacts(page: Page): Promise<string> {
  const result = await page.evaluate(() =>
    (window as unknown as Bridge).electronAPI.runtime.executeTool('memory_console', { action: 'facts', limit: 20 }),
  )
  return JSON.stringify(result)
}

function outgoingSystem(llm: FakeOllama, userText: string): string {
  const body = llm.requests
    .map((request) => request.body as { messages?: { role: string; content: string }[] })
    .find((request) => request.messages?.some((message) => message.role === 'user' && message.content === userText))
  expect(body, 'turno de chat real enviado ao provedor').toBeDefined()
  return body?.messages?.filter((message) => message.role === 'system').map((message) => message.content).join('\n') ?? ''
}

test('fato dito no chat é extraído, gravado e volta no prompt do turno seguinte', async ({ app, llm }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => page.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady))
    .toBe(true)

  // Sem fato confirmado: nada entra na memória de longo prazo.
  await send(page, 'hoje o dia foi corrido demais')
  const extractionCalls = () => llm.requests.filter((r) => JSON.stringify(r.body).includes('Extract PERMANENT, CONFIRMED facts')).length
  await expect.poll(extractionCalls, { timeout: 30_000 }).toBeGreaterThanOrEqual(1)
  expect(await knownFacts(page)).not.toContain('jogo favorito')

  await send(page, 'Meu jogo favorito é Hades.')
  await expect.poll(() => knownFacts(page), { timeout: 30_000 }).toContain('jogo favorito é Hades')

  const question = 'lembra qual jogo eu mais curto?'
  await send(page, question)
  const system = outgoingSystem(llm, question)
  expect(system).toContain('## What You Know About The User')
  expect(system).toMatch(/^- jogo favorito é Hades$/m)

  writeArtifact('long-term-memory', {
    extractionCalls: extractionCalls(),
    storedFact: 'jogo favorito é Hades',
    injectedIntoNextTurnPrompt: true,
    noFactFromSmallTalk: true,
  })
})
