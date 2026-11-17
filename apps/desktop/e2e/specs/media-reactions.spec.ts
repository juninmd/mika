import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import type { FakeOllama } from '../fixtures/fake-ollama-server'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

// Real-time reactions (`vision::media_reaction`): with the opt-in on and a video in focus, the watcher
// captures one frame, the vision model describes it, Mika reacts through the proactive engine and the
// edge dock shows a "Watching" card. With the opt-in off she stays silent.
type Runtime = { saveSetting(path: string, value: unknown): Promise<unknown> }

const VIDEO = { title: 'Trailer GTA VI - YouTube — Mozilla Firefox', owner: 'firefox' }
// The request body is JSON, so the quote arrives escaped.
const REACTION = 'The user is watching a video: \\"Trailer GTA VI'
const FRAME = 'This is a frame of a video the user is watching'
// The watcher polls every 20 s and needs two polls on the same video.
const REACTION_BUDGET_MS = 75_000

const save = (page: Page, path: string, value: unknown) =>
  page.evaluate(
    ([p, v]) => (window as unknown as { electronAPI: { runtime: Runtime } }).electronAPI.runtime.saveSetting(p, v),
    [path, value] as const,
  )

const sent = (llm: FakeOllama, needle: string) => llm.requests.filter((r) => JSON.stringify(r.body).includes(needle)).length

test('reacts to the video in focus only after the opt-in, and shows it on the dock', async ({ app, llm }) => {
  test.setTimeout(240_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await save(page, 'privacy.screenCapture', true)
  await save(page, 'vision.enabled', true)
  await save(page, 'app.miniModeStyle', 'dock-right')
  await save(page, 'app.miniMode', true)
  await callE2eHook(page, 'set-active-window', VIDEO)

  // Opt-in off (the default): two full polls on the video, no reaction.
  await page.waitForTimeout(45_000)
  expect(sent(llm, REACTION), 'no reaction without the opt-in').toBe(0)

  await save(page, 'vision.mediaReactions', true)
  await expect.poll(() => sent(llm, REACTION), { timeout: REACTION_BUDGET_MS, intervals: [2_000] }).toBe(1)
  const card = page.getByRole('article').filter({ hasText: 'Trailer GTA VI' })
  await expect(card).toBeVisible({ timeout: 15_000 })
  await expect(card).toContainText('Assistindo')
  await page.screenshot({ path: artifactPath('media-reaction-dock-card.png') })
  const dockCard = (await card.innerText()).split('\n').filter(Boolean)

  // Cooldown (default 5 min): the same video in focus brings no second reaction.
  await page.waitForTimeout(45_000)
  expect(sent(llm, REACTION), 'one reaction per cooldown').toBe(1)

  writeArtifact('media-reaction', {
    activeWindow: VIDEO,
    reactionsWithOptInOff: 0,
    reactionsWithOptInOn: sent(llm, REACTION),
    frameDescribedByVisionModel: sent(llm, FRAME) > 0,
    dockCard,
  })
})
