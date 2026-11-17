import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import type { FakeOllama } from '../fixtures/fake-ollama-server'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }

// Modo co-piloto (`gamer::copilot`): a análise de visão marca um perigo/item perdido no bloco
// `copilot` do `GAME_SIGNAL`, o watcher publica `GameCopilotAlert`, e a Mika gera um aviso curto.
// Estímulo: `vision-ingest` (o tick REAL do watcher, sem captura). Observação: o pedido HTTP que o
// LLM dublê recebeu com o prompt `copilot-alert.md` — a fala sai por TTS, desligado no E2E.

type Alert = { kind: 'danger' | 'missed_item'; detail: string }

function rawAnalysis(alert: Alert): string {
  const signal = {
    isLikelyGame: true,
    game: 'Elden Ring',
    gameMoment: null,
    tense: false,
    copilot: alert,
    isFunny: false,
    people: [],
    platformContext: '',
    memeContext: '',
    newFacts: [],
  }
  return `Jogador atravessando a ponte.\nGAME_SIGNAL: ${JSON.stringify(signal)}`
}

function copilotPrompts(llm: FakeOllama, detail: string): number {
  return llm.requests.filter((r) => {
    const body = JSON.stringify(r.body)
    return body.includes('co-pilot') && body.includes(detail)
  }).length
}

async function ingest(page: Page, alert: Alert) {
  await callE2eHook(page, 'vision-ingest', { raw: rawAnalysis(alert) })
}

const quiet = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

test('co-piloto avisa de perigo e item perdido só quando ligado, sem repetir o mesmo aviso', async ({ app, llm }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => page.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady))
    .toBe(true)

  const archer: Alert = { kind: 'danger', detail: 'archer on the roof' }
  await ingest(page, archer)
  await quiet(3_000)
  const whileOff = copilotPrompts(llm, archer.detail)
  expect(whileOff, 'desligado por padrão: nenhum aviso chega ao LLM').toBe(0)

  const opened = app.waitForEvent('window')
  await page.evaluate(() => (window as unknown as Bridge).electronAPI.shell.openSettings())
  const settings = await opened
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('Gamer Companion')
  await settings.keyboard.press('Enter')
  const toggle = settings.getByRole('switch', { name: /Modo co-piloto/ })
  await expect(toggle).toBeVisible()
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await toggle.scrollIntoViewIfNeeded()
  await settings.screenshot({ path: artifactPath('game-copilot-toggle.png') })
  await expect
    .poll(() =>
      settings.evaluate(async () => {
        const { config } = await (window as unknown as Bridge).electronAPI.runtime.getSettingsConfig()
        return (config['gamer-companion'] as { copilotEnabled?: boolean }).copilotEnabled
      }),
    )
    .toBe(true)

  await ingest(page, archer)
  await expect.poll(() => copilotPrompts(llm, archer.detail), { timeout: 10_000 }).toBe(1)

  await ingest(page, archer)
  await quiet(3_000)
  const afterRepeat = copilotPrompts(llm, archer.detail)
  expect(afterRepeat, 'o mesmo aviso não volta').toBe(1)

  // Modo passageiro ligado: o co-piloto continua avisando (nada dele vai para o disco).
  await settings.evaluate(() => (window as unknown as Bridge).electronAPI.runtime.saveSetting('privacy.passengerMode', true))
  await quiet(8_500)
  const chest: Alert = { kind: 'missed_item', detail: 'chest by the bridge' }
  await ingest(page, chest)
  await expect.poll(() => copilotPrompts(llm, chest.detail), { timeout: 10_000 }).toBe(1)

  writeArtifact('game-copilot', {
    offByDefault: { alertsSentToLlm: whileOff },
    toggleOn: { config: 'gamer-companion.copilotEnabled = true' },
    dangerAlert: { detail: archer.detail, llmCalls: 1 },
    sameAlertAgain: { llmCalls: afterRepeat },
    passengerMode: { detail: chest.detail, llmCalls: copilotPrompts(llm, chest.detail) },
  })
})
