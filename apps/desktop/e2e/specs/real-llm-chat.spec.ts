import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { waitForUiIdle } from '../fixtures/settled'

// Smoke MANUAL contra o Ollama de verdade — nunca na suíte determinística (§5.3: LLM dublê, sem rede).
//   MIKA_E2E_REAL_OLLAMA=qwen3.5:4b pnpm exec playwright test -c e2e/playwright.config.ts e2e/specs/real-llm-chat.spec.ts
// Prova o que o dublê não prova: o modelo padrão do wizard responde pelo caminho inteiro (Rust -> Ollama
// -> stream -> bolha), em português, sem eco do prompt de sistema.
const REAL_MODEL = process.env.MIKA_E2E_REAL_OLLAMA

type Bridge = {
  electronAPI?: {
    runtime: {
      onAssistantMessage(cb: (m: { content: string }) => void): void
      onChatError(cb: (e: { message: string }) => void): void
      saveSetting(path: string, value: unknown): Promise<unknown>
      executeTool(
        name: string,
        args: Record<string, unknown>,
      ): Promise<{ success: boolean; error?: string; data?: { response?: string } } | undefined>
    }
  }
}

function waitForReply(page: Page, timeoutMs: number): Promise<{ content: string }> {
  return page.evaluate((ms) => {
    const api = (window as unknown as Bridge).electronAPI?.runtime
    return new Promise<{ content: string }>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`sem resposta do modelo real em ${ms / 1000}s`)), ms)
      api?.onAssistantMessage((m) => {
        clearTimeout(timer)
        resolve({ content: m.content })
      })
      api?.onChatError((e) => {
        clearTimeout(timer)
        reject(new Error(e.message))
      })
    })
  }, timeoutMs)
}

test.skip(!REAL_MODEL, 'defina MIKA_E2E_REAL_OLLAMA=<modelo> (Ollama em 127.0.0.1:11434) para rodar')

test('chat com o modelo real do Ollama responde em português', async ({ app }) => {
  test.setTimeout(240_000)
  const page = await app.firstWindow()
  const pending = waitForReply(page, 180_000)
  const composer = await openComposer(page)
  await composer.fill('Oi Mika! Em uma frase curta, o que você faz?')
  await composer.press('Enter')

  const reply = await pending
  expect(reply.content.trim().length).toBeGreaterThan(8)
  expect(reply.content).not.toMatch(/fake|dublê|HOMOLOGA_/i)
  await waitForUiIdle(page)
  await page.screenshot({ path: join(artifactPath('real-llm-chat.png')) })
  writeArtifact('real-llm-chat', { model: REAL_MODEL, replyChars: reply.content.length, reply: reply.content.slice(0, 280) })
})

// A visão com o modelo real (`qwen3.5:4b` tem visão): captura de monitor de verdade + LLM de verdade.
// O usuário consentiu em Privacidade e ligou a Visão; a janela ativa é um jogo (a Mika nunca olha a própria).
test('visão: captura a tela de verdade e o modelo real descreve', async ({ app }) => {
  test.setTimeout(300_000)
  const page = await app.firstWindow()
  await page.evaluate(async () => {
    const api = (window as unknown as Bridge).electronAPI?.runtime
    await api?.saveSetting('privacy.screenCapture', true)
    await api?.saveSetting('vision.enabled', true)
  })
  await callE2eHook(page, 'set-active-window', { title: 'Hollow Knight', owner: 'hollow_knight' })
  const watch = await page.evaluate(
    () =>
      (window as unknown as Bridge).electronAPI?.runtime.executeTool('vision_capture', {
        action: 'watch_now',
        question: 'Em uma frase, o que você vê na tela?',
      }),
  )
  expect(watch?.error ?? null).toBeNull()
  expect(watch?.success).toBe(true)
  const response = String(watch?.data?.response ?? '')
  expect(response.trim().length).toBeGreaterThan(8)
  await waitForUiIdle(page)
  await page.screenshot({ path: artifactPath('real-vision-watch-now.png') })
  // A resposta descreve a tela REAL de quem roda o smoke: o artefato guarda só o tamanho, nunca o texto.
  writeArtifact('real-vision-watch-now', { model: REAL_MODEL, responseChars: response.length, describedRealScreen: true })
})
