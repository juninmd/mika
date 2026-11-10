import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import type { FakeOllama } from '../fixtures/fake-ollama-server'
import { expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }

// Idioma da MENSAGEM (`brain::reply_language`): app em pt-BR, mensagem inteira em inglês → a nota
// "Reply language" sai no prompt de sistema real enviado por HTTP; mensagem em português não muda
// nada; e o toggle de Configurações → LLM desliga o comportamento de verdade.

const ENGLISH = 'Can you help me find a good build for my warrior in this game?'
const PORTUGUESE = 'Você pode me ajudar a achar uma build boa pro meu guerreiro nesse jogo?'

async function send(page: Page, content: string) {
  const reply = await page.evaluate((text) => (window as unknown as Bridge).electronAPI.runtime.sendChatMessage(text), content)
  expect(reply.content.trim()).not.toBe('')
}

/**
 * O prompt de sistema do turno de CHAT com `userText` enviado a partir de `from` (índice em
 * `llm.requests`). O primeiro casamento é o turno; os seguintes são o pós-turno (extração de
 * pessoas/fatos, `## CURRENT TASK`), que reenvia a mesma mensagem com outro prompt.
 */
function outgoingSystem(llm: FakeOllama, userText: string, from: number): string {
  const body = llm.requests
    .slice(from)
    .map((request) => request.body as { messages?: { role: string; content: string }[] })
    .find((request) => request.messages?.some((message) => message.role === 'user' && message.content === userText))
  expect(body, 'turno de chat real enviado ao provedor').toBeDefined()
  return body?.messages?.filter((message) => message.role === 'system').map((message) => message.content).join('\n') ?? ''
}

test('Mika responde no idioma em que a mensagem foi escrita, e o toggle desliga isso', async ({ app, llm }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => page.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady))
    .toBe(true)

  let from = llm.requests.length
  await send(page, ENGLISH)
  const english = outgoingSystem(llm, ENGLISH, from)
  expect(english).toContain('## Reply language')
  expect(english).toContain('The user wrote this message in **English (US)**')

  from = llm.requests.length
  await send(page, PORTUGUESE)
  expect(outgoingSystem(llm, PORTUGUESE, from)).not.toContain('## Reply language')

  const opened = app.waitForEvent('window')
  await page.evaluate(() => (window as unknown as Bridge).electronAPI.shell.openSettings())
  const settings = await opened
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('LLM')
  await settings.keyboard.press('Enter')
  const toggle = settings.getByRole('switch', { name: /Responder no idioma da mensagem/ })
  await expect(toggle).toBeVisible()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await toggle.scrollIntoViewIfNeeded()
  await settings.screenshot({ path: artifactPath('reply-language-toggle.png') })

  await toggle.click()
  await expect
    .poll(() =>
      settings.evaluate(async () => {
        const result = await (window as unknown as Bridge).electronAPI.runtime.getSettingsConfig()
        return (result.config.llm as { matchUserLanguage?: boolean }).matchUserLanguage
      }),
    )
    .toBe(false)
  from = llm.requests.length
  await send(page, ENGLISH)
  expect(outgoingSystem(llm, ENGLISH, from)).not.toContain('## Reply language')

  writeArtifact('reply-language', {
    englishInPtBrApp: 'nota "Reply language" com English (US)',
    portugueseInPtBrApp: 'sem nota',
    toggleOff: 'sem nota',
  })
})
