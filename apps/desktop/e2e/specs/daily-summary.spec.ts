import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

type Api = {
  electronAPI: {
    runtime: {
      saveSetting: (path: string, value: unknown) => Promise<unknown>
      onToolResult: (cb: (p: { name: string; result: unknown }) => void) => () => void
    }
    shell: { window: { setMiniMode: (on: boolean) => Promise<unknown> } }
  }
}

const saveSetting = (page: Page, path: string, value: unknown) =>
  page.evaluate(([p, v]) => (window as unknown as Api).electronAPI.runtime.saveSetting(p as string, v), [path, value])

/** Every `game_session` result the chat produces, kept on `window` so the spec can read them back. */
async function recordToolResults(page: Page): Promise<void> {
  await page.evaluate(() => {
    const store = window as unknown as Api & { __today: unknown[] }
    store.__today = []
    store.electronAPI.runtime.onToolResult(({ name, result }) => {
      if (name === 'game_session') store.__today.push(typeof result === 'string' ? JSON.parse(result) : result)
    })
  })
}

const todayResults = (page: Page) =>
  page.evaluate(() => (window as unknown as { __today: Array<{ data: Record<string, unknown> }> }).__today)

/** Taps "what did we do today?" and waits for THIS ask's tool result, never the previous one. */
async function askToday(page: Page): Promise<Record<string, unknown>> {
  const before = (await todayResults(page)).length
  await clickDockButton(page, 'O que fizemos hoje?')
  await expect.poll(async () => (await todayResults(page)).length, { timeout: 30_000 }).toBeGreaterThan(before)
  return (await todayResults(page)).at(-1)?.data ?? {}
}

// Real path end to end: the watcher sees a game window, a vision moment lands in the session, the
// session ends and gets its LLM recap, then the dock's "what did we do today?" asks the chat, the LLM
// double calls `game_session` today and the card shows up. Passenger mode keeps the stats, drops the recap.
test('resumo do dia: sessão real, conquista, cartão no dock e modo passageiro', async ({ app }) => {
  test.setTimeout(240_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await saveSetting(page, 'gamer-companion.gameWatcherIntervalSeconds', 10)
  await recordToolResults(page)

  await callE2eHook(page, 'set-active-window', { title: 'Hollow Knight', owner: 'hollowknight' })
  const moment = (type: string, description: string) =>
    callE2eHook(page, 'publish-event', {
      eventKey: 'steam:game-moment',
      payload: { game: 'Hollow Knight', type, description, source: 'vision', at: Date.now() },
    })
  await saveSetting(page, 'app.miniModeStyle', 'dock-right')
  await page.evaluate(() => (window as unknown as Api).electronAPI.shell.window.setMiniMode(true))

  // The watcher re-reads its interval only after the current sleep (up to 30 s + 10 s), and a moment
  // only lands once the session is open: keep sending until the day summary sees it.
  await expect
    .poll(
      async () => {
        // `victory`: the vision prompt's own vocabulary (`boss|cutscene|death|victory|defeat`).
        await moment('victory', 'Derrotou o Hornet')
              const last = await askToday(page)
        return (last.achievements as unknown[] | undefined)?.length ?? 0
      },
      { timeout: 90_000, intervals: [5_000] },
    )
    .toBeGreaterThan(0)
  await moment('death', 'Caiu nos espinhos')

  // Leave the game: the session closes and the LLM recap is written in the background.
  await callE2eHook(page, 'set-active-window', { title: 'Terminal', owner: 'xterm' })
  await expect
    .poll(
      async () => {
        const last = await askToday(page)
        return (last.highlights as unknown[] | undefined)?.length ?? 0
      },
      { timeout: 60_000, intervals: [5_000] },
    )
    .toBeGreaterThan(0)

  const card = page.locator('article', { hasText: 'Resumo do dia' }).last()
  await expect(card).toBeVisible()
  await expect(card).toContainText('Hollow Knight')
  await expect(card).toContainText('Derrotou o Hornet')
  await expect(card).toContainText('Mortes: 1')
  await page.screenshot({ path: artifactPath('daily-summary-card.png') })
  const normal = (await todayResults(page)).at(-1)?.data
  expect(normal?.deaths, 'a tela de morte repetida a cada passada conta uma vez').toBe(1)

  await clickDockButton(page, 'Modo passageiro: nada desta sessão é guardado')
  const passenger = await askToday(page)
  const passengerCard = page.locator('article', { hasText: 'modo passageiro' }).last()
  await expect(passengerCard).toBeVisible({ timeout: 30_000 })
  await expect(passengerCard).toContainText('Derrotou o Hornet')
  await page.screenshot({ path: artifactPath('daily-summary-passenger.png') })

  expect(passenger.passengerMode).toBe(true)
  expect(passenger.highlights, 'passageiro: sem lembrança').toBeUndefined()
  expect(passenger.totalMs).toBe(normal?.totalMs)
  writeArtifact('daily-summary', { normal, passenger })
})
