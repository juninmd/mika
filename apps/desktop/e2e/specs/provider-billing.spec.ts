import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { ELEVENLABS_TEST_KEY } from '../fixtures/fake-elevenlabs'
import { expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }

// Créditos nas telas de provider: saldo pela API oficial onde existe (OpenRouter `/api/v1/key`,
// ElevenLabs `/v1/user/subscription`, ambas servidas pelo dublê local — sem nuvem nem chave real) e
// "não disponível pela API" onde o provedor não publica saldo (Groq), sem inventar número.

const save = (page: Page, path: string, value: unknown) =>
  page.evaluate(([p, v]) => (window as unknown as Bridge).electronAPI.runtime.saveSetting(p as string, v), [path, value])

const billing = (page: Page, provider: string) =>
  page.evaluate((p) => (window as unknown as Bridge).electronAPI.runtime.getProviderBilling(p), provider)

async function openTab(settings: Page, query: string) {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
}

async function cardFor(settings: Page, provider: string, verdict: string) {
  const card = settings.locator(`[data-testid="provider-billing"][data-provider="${provider}"]`)
  await expect(card).toHaveAttribute('data-verdict', verdict, { timeout: 15_000 })
  await card.scrollIntoViewIfNeeded()
  return card
}

test('telas de provider mostram créditos, cota e uso — ou dizem que a API não informa', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await save(page, 'apiKeys.openrouter', 'sk-or-v1-chave-de-teste-nunca-real')
  await save(page, 'llm.provider', 'openrouter')
  await save(page, 'apiKeys.elevenLabs', ELEVENLABS_TEST_KEY)
  await save(page, 'voice.ttsProvider', 'elevenlabs')
  await save(page, 'voice.groqApiKey', 'gsk_chave-de-teste-nunca-real')
  await save(page, 'voice.asrProvider', 'groq-whisper')

  const local = await billing(page, 'ollama')
  expect(local.api.status).toBe('notApplicable')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await openTab(settings, 'LLM')
  const llm = await cardFor(settings, 'openrouter', 'low')
  await expect(llm.getByText('Saldo baixo')).toBeVisible()
  await expect(llm.getByText(/^US\$\s?0,40$/)).toBeVisible()
  await expect(llm.getByRole('meter', { name: 'Saldo restante' })).toHaveAttribute('value', '4')
  await llm.screenshot({ path: artifactPath('provider-billing-llm.png') })

  await openTab(settings, 'Fala')
  const tts = await cardFor(settings, 'elevenlabs', 'ok')
  await expect(tts.getByText('27.500 caracteres', { exact: true })).toBeVisible()
  await expect(tts.getByRole('meter', { name: 'Saldo restante' })).toHaveAttribute('value', '92')
  await tts.screenshot({ path: artifactPath('provider-billing-tts.png') })

  await openTab(settings, 'Escuta')
  const stt = await cardFor(settings, 'groq-whisper', 'unknown')
  await expect(stt.getByText(/Saldo não disponível pela API/)).toBeVisible()
  await stt.screenshot({ path: artifactPath('provider-billing-stt.png') })

  writeArtifact('provider-billing', {
    openrouter: (await billing(page, 'openrouter')).api,
    elevenlabs: (await billing(page, 'elevenlabs')).api,
    groq: (await billing(page, 'groq-whisper')).api.status,
    ollama: local.api.status,
  })
})
