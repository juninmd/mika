import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { getLLMProviderEntry } from '../../../../packages/sdk/src/llm/llm-catalog-data'
import { openComposer } from '../fixtures/composer'
import { writeArtifact } from '../fixtures/artifact'
import { CLOUD_REPLY } from '../fixtures/fake-ollama-server'
import { snapTop, waitForUiIdle } from '../fixtures/settled'
import { expect, test } from '../fixtures/tauri-app'

// Gemini e OpenRouter pelo caminho REAL do app (resolução de modelo, chave, wire do provedor, stream,
// bolha) contra o dublê local — sem chamar nem cobrar a nuvem. O que prova: o modelo PADRÃO do catálogo
// (o que o wizard oferece) é exatamente o que o backend manda ao provedor.
type Bridge = {
  electronAPI?: {
    runtime: {
      onAssistantMessage(cb: (m: { content: string }) => void): void
      onChatError(cb: (e: { message: string }) => void): void
      saveSetting(path: string, value: unknown): Promise<unknown>
    }
  }
}

function waitForReply(page: Page): Promise<{ content: string }> {
  return page.evaluate(
    () =>
      new Promise<{ content: string }>((resolve, reject) => {
        const api = (window as unknown as Bridge).electronAPI?.runtime
        const timer = setTimeout(() => reject(new Error('sem resposta do provedor de mentirinha em 30s')), 30_000)
        api?.onAssistantMessage((m) => {
          clearTimeout(timer)
          resolve({ content: m.content })
        })
        api?.onChatError((e) => {
          clearTimeout(timer)
          reject(new Error(e.message))
        })
      }),
  )
}

const PROVIDERS = [
  { id: 'google', keyPath: 'apiKeys.googleAI', wirePath: (model: string) => `/v1beta/models/${model}:streamGenerateContent` },
  { id: 'openrouter', keyPath: 'apiKeys.openrouter', wirePath: () => '/api/v1/chat/completions' },
] as const

test('Gemini e OpenRouter respondem pelo modelo padrão do catálogo, sem tocar a nuvem', async ({ app, llm }) => {
  test.setTimeout(180_000)
  const page = await app.firstWindow()
  const shots = join(process.cwd(), 'e2e/screenshots')
  const seen: Record<string, { model: string; wirePath: string }> = {}

  for (const provider of PROVIDERS) {
    const { defaultModel } = getLLMProviderEntry(provider.id)
    await page.evaluate(
      async ([id, keyPath]) => {
        const api = (window as unknown as Bridge).electronAPI?.runtime
        await api?.saveSetting('llm.provider', id)
        await api?.saveSetting(keyPath as string, 'chave-de-teste-nunca-real')
      },
      [provider.id, provider.keyPath],
    )

    const before = llm.requests.length
    const pending = waitForReply(page)
    const composer = await openComposer(page)
    await composer.fill('Oi Mika! Em uma frase, quem é você?')
    await composer.press('Enter')

    const reply = await pending
    expect(reply.content).toContain(CLOUD_REPLY.slice(0, 30))
    await waitForUiIdle(page)

    const wirePath = provider.wirePath(defaultModel)
    const hit = llm.requests.slice(before).find((r) => r.path === wirePath)
    expect(hit, `${provider.id}: nenhuma chamada em ${wirePath}`).toBeTruthy()
    if (provider.id === 'openrouter') {
      expect((hit?.body as { model?: string }).model).toBe(defaultModel)
    }

    await snapTop(page, join(shots, `cloud-mock-${provider.id}.png`))
    seen[provider.id] = { model: defaultModel, wirePath }
  }

  writeArtifact('cloud-providers-mock', { providers: seen, realCloudCalled: false })
})
