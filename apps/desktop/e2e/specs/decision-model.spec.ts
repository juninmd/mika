import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { startFakeDecision } from '../fixtures/fake-decision-server'
import type { FakeOllama } from '../fixtures/fake-ollama-server'
import { clickDockButton } from '../fixtures/dock'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

// Decision-model pre-filter (`vision::decision`): turned on from the Vision tab, the watcher asks the
// local decision server about every frame and only pays for the vision LLM when the frame matters.
// A calm frame never reaches the LLM; an urgent one does. The watcher only looks at a frame that
// changed, and the test display does not change, so each verdict gets its own app run.
type Runtime = { saveSetting(path: string, value: unknown): Promise<unknown> }

const TICK_PROMPT = 'Periodic screen observation'
// The watcher ticks every 60 s (`vision.captureIntervalMs` floor) outside a game.
const TICK_BUDGET_MS = 150_000

const save = (page: Page, path: string, value: unknown) =>
  page.evaluate(
    ([p, v]) => (window as unknown as { electronAPI: { runtime: Runtime } }).electronAPI.runtime.saveSetting(p, v),
    [path, value] as const,
  )

const ticks = (llm: FakeOllama) => llm.requests.filter((r) => JSON.stringify(r.body).includes(TICK_PROMPT)).length

async function openVisionTab(page: Page): Promise<Page> {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('Visão')
  await settings.keyboard.press('Enter')
  return settings
}

test('turned on from the Vision tab, a calm frame never reaches the vision LLM', async ({ app, llm }) => {
  test.setTimeout(420_000)
  const decision = await startFakeDecision()
  try {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await save(page, 'privacy.screenCapture', true)
    await save(page, 'vision.codeAwareness', false)
    // The watcher stays idle until the decision server is set, so no tick races the UI steps.
    await save(page, 'vision.visionAssistActive', false)
    await save(page, 'vision.enabled', true)
    await callE2eHook(page, 'set-active-window', { title: 'Xonotic', owner: 'xonotic' })

    const settings = await openVisionTab(page)
    const toggle = settings.getByRole('switch', { name: 'Filtrar a tela com o modelo de decisão' })
    await expect(toggle).toHaveAttribute('aria-checked', 'false')
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true')
    const url = settings.getByLabel('Endereço do servidor')
    await url.fill(decision.baseUrl)
    await url.press('Enter')
    await save(page, 'vision.visionAssistActive', true)

    // Calm frames: the decision server is asked, the vision LLM never is.
    await expect.poll(() => decision.requests, { timeout: TICK_BUDGET_MS, intervals: [2_000] }).toBeGreaterThan(0)
    await expect(settings.getByText('Sem falhas')).toBeVisible({ timeout: 15_000 })
    await toggle.scrollIntoViewIfNeeded()
    await settings.screenshot({ path: artifactPath('decision-model-card.png') })
    const calmRequests = decision.requests
    const calmTicks = ticks(llm)
    expect(calmTicks, 'calm frame never pays for the vision LLM').toBe(0)

    writeArtifact('decision-model-calm', {
      decisionServer: decision.baseUrl,
      calmFramesAsked: calmRequests,
      visionLlmCallsWhileCalm: calmTicks,
    })
  } finally {
    await decision.close()
  }
})

test('an urgent frame reaches the vision LLM', async ({ app, llm }) => {
  test.setTimeout(300_000)
  const decision = await startFakeDecision()
  decision.verdict = 'urgent'
  try {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await save(page, 'privacy.screenCapture', true)
    await save(page, 'vision.codeAwareness', false)
    await save(page, 'vision.decisionEnabled', true)
    await save(page, 'vision.decisionBaseUrl', decision.baseUrl)
    await save(page, 'vision.enabled', true)
    await callE2eHook(page, 'set-active-window', { title: 'Xonotic', owner: 'xonotic' })

    await expect.poll(() => ticks(llm), { timeout: TICK_BUDGET_MS, intervals: [2_000] }).toBeGreaterThan(0)
    expect(decision.requests, 'the decision model saw the frame first').toBeGreaterThan(0)

    writeArtifact('decision-model-urgent', {
      decisionServer: decision.baseUrl,
      framesAsked: decision.requests,
      visionLlmCalls: ticks(llm),
    })
  } finally {
    await decision.close()
  }
})
