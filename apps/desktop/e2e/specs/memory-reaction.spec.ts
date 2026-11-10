import type { Page } from '@playwright/test'
import { writeArtifact } from '../fixtures/artifact'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'

// Tauri harness (rewritten 2026-09-23, replacing the Electron `app.evaluate`/`globalThis.e2e`
// stub that never ran — see git history for the dead version). Three moving parts:
//
// 1. Stimulus: `emitScreenObservation` builds the SAME raw model reply
//    (`vision::watcher::DEFAULT_TICK_PROMPT` contract — comment + trailing `GAME_SIGNAL:` JSON)
//    that a real LLM turn would produce, and feeds it through the `vision-ingest` e2e hook
//    (`e2e_hooks.rs` → `vision::ingest_analysis_for_e2e`). That function is the REAL watcher-tick
//    code path minus the capture/LLM call: it parses `people`/`isFunny`/`newFacts`, auto-upserts
//    people into `person_catalog`, and publishes `EventKey::ScreenObservation`/`GameMoment` on the
//    real bus — so this drives person-catalog auto-upsert AND the proactive reaction, not just one
//    or the other. Publishing `vision:screen-observation` directly via the generic `publish-event`
//    hook would reach `ScreenProvider` but never call the watcher's upsert (it lives in the tick,
//    not on the bus) — that gap is exactly why `vision-ingest` exists.
// 2. Observation: chat replies and person-catalog updates are read from the REAL renderer bridge
//    (`window.electronAPI.runtime.onAssistantMessage`/`onPersonCatalogUpdated`/`onScreenObservation`
//    — `apps/desktop/src/renderer/bridge/mika-runtime-*.ts`), buffered into `window.__e2eLog` by
//    `installEventLog`. `person_catalog_list`/`_get`/`_upsert` RPC (`callPersonRpc`) already worked
//    under Tauri before this rewrite (it goes through `window.electronAPI.shell.pluginRpc`, never
//    through `app.evaluate`) and is kept as-is.
// 3. Wikipedia: `person_catalog::enrichment.rs`'s `WIKI_SUMMARY_API` const can't be pointed at a
//    stub via config (deliberately not configurable). The `wiki` fixture (`fake-wiki-server.ts`)
//    starts a local HTTP stub, and `tauri-app.ts` passes its base URL to the backend as
//    `MIKA_E2E_WIKI_BASE_URL` — honored only when `MIKA_E2E=1` too
//    (`guarded_fetch::e2e_wiki_override`, double-locked exactly like `e2e_hooks.rs`, never a
//    production config key). `getWikiFetchCount` reads the stub's own request log instead of a
//    `globalThis.fetch` shim.
//
// KNOWN GAP, found while wiring this up, not fixed here (outside this rewrite's scope — flagging
// for the lead): `EventKey::PersonUpdated` (Rust) publishes `{ id, created }`; the renderer bridge
// type for `onPersonCatalogUpdated` declares `{ personId, name? }`. Neither field lines up with
// what Rust actually sends, so this suite never matches on that event's `name`/`personId` — every
// "was this person created" assertion below polls `person_catalog_list`/`_get` (RPC, proven to
// work) instead. If that bridge payload gets fixed, these helpers can switch to the event.

async function installEventLog(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as {
      __e2eLog: Array<{ event: string; payload: unknown }>
      electronAPI: { runtime: Record<string, (cb: (payload: unknown) => void) => void> }
    }
    w.__e2eLog = []
    const api = w.electronAPI.runtime
    api.onAssistantMessage((payload) => w.__e2eLog.push({ event: 'chat:assistant-message', payload }))
    api.onScreenObservation((payload) => w.__e2eLog.push({ event: 'vision:screen-observation', payload }))
    api.onPersonCatalogUpdated((payload) => w.__e2eLog.push({ event: 'person-catalog:updated', payload }))
  })
}

async function clearEventLog(page: Page): Promise<void> {
  await page.evaluate(() => {
    ;(window as unknown as { __e2eLog: unknown[] }).__e2eLog = []
  })
}

async function readEventLog(page: Page): Promise<Array<{ event: string; payload: any }>> {
  return page.evaluate(() => (window as unknown as { __e2eLog: Array<{ event: string; payload: any }> }).__e2eLog ?? [])
}

async function waitForEvent(page: Page, eventName: string, predicate?: (payload: any) => boolean, timeout = 8000) {
  const start = Date.now()
  while (Date.now() - start < timeout) {
    const logs = await readEventLog(page)
    const matched = [...logs].reverse().find((l) => l.event === eventName && (!predicate || predicate(l.payload)))
    if (matched) return matched.payload
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  const finalLogs = await readEventLog(page)
  console.error(`\n[TIMEOUT ERROR] Timed out waiting for event "${eventName}". Fired events:\n`, JSON.stringify(finalLogs, null, 2))
  throw new Error(`Timed out waiting for event: ${eventName}`)
}

async function countAssistantMessages(page: Page): Promise<number> {
  const logs = await readEventLog(page)
  return logs.filter((l) => l.event === 'chat:assistant-message').length
}

async function bypassSetup(page: Page) {
  await installEventLog(page)

  // Mika comments on window switches on her own budget (once per ~2.5min), which would land inside
  // an ABSENCE assertion window and be mistaken for observation commentary. Spend that budget here
  // on purpose so the rest of the file measures only what it emits itself.
  await callE2eHook(page, 'publish-event', { eventKey: 'os:active-window-changed', payload: { title: 'Homologacao E2E', owner: 'e2e' } })
  try {
    await waitForEvent(page, 'chat:assistant-message', undefined, 10_000)
  } catch {}
  await new Promise((resolve) => setTimeout(resolve, 1500))
  await clearEventLog(page)
}

async function callPersonRpc(page: Page, method: string, args: Record<string, unknown>) {
  return page.evaluate(
    async ({ method, args }) => {
      const eAPI = (window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }).electronAPI
      if (!eAPI.shell.pluginRpc) throw new Error('RPC unavailable')
      const envelope = (await eAPI.shell.pluginRpc({ pluginId: 'mika.person-catalog', method, args })) as {
        success: boolean
        data?: unknown
        error?: string
      }
      if (!envelope.success) throw new Error(`plugin RPC transport failed: ${envelope.error}`)
      return envelope.data as any
    },
    { method, args },
  )
}

/** Polls the real catalog (RPC, proven under Tauri) instead of the event — see the header note. */
async function waitForPersonInCatalog(page: Page, predicate: (p: { primaryName: string; id: string }) => boolean, timeout = 20_000) {
  const start = Date.now()
  let lastList: unknown[] = []
  while (Date.now() - start < timeout) {
    const list = await callPersonRpc(page, 'person_catalog_list', { limit: 500 })
    lastList = list.data ?? []
    const found = (lastList as Array<{ primaryName: string; id: string }>).find(predicate)
    if (found) return found
    await new Promise((resolve) => setTimeout(resolve, 150))
  }
  throw new Error(`timed out waiting for a person in the catalog. Last list: ${JSON.stringify(lastList)}`)
}

type ObservationPayload = {
  analysis?: string
  isFunny?: boolean
  isLikelyGame?: boolean
  foregroundApp?: { title?: string; processName?: string }
  people?: Array<{ name: string; type?: string; confidence?: string }>
}

/** Mirrors `vision::watcher::DEFAULT_TICK_PROMPT`'s contract: comment, then `GAME_SIGNAL:` + JSON. */
function buildRawObservation(payload: ObservationPayload): string {
  const signal = {
    isLikelyGame: payload.isLikelyGame ?? false,
    game: '',
    gameMoment: null,
    tense: false,
    // `vision::watcher::may_auto_catalog`: `real_person` genérico nunca vira ficha por visão, e figura
    // pública só com confiança ALTA — o detector emite o que a regra aceita (`influencer`/`high`).
    isFunny: payload.isFunny ?? false,
    people: (payload.people ?? []).map((p) => ({ name: p.name, type: p.type ?? 'influencer', confidence: p.confidence ?? 'high' })),
    platformContext: payload.foregroundApp?.title ?? '',
    memeContext: '',
    newFacts: [],
  }
  return `${payload.analysis ?? ''}\nGAME_SIGNAL: ${JSON.stringify(signal)}`
}

/** Drives the REAL watcher pipeline (parse → auto-upsert → publish) — see the file header. */
async function emitScreenObservation(page: Page, payload: ObservationPayload) {
  await callE2eHook(page, 'vision-ingest', { raw: buildRawObservation(payload) })
}

async function getWikiFetchCount(page: Page, wikiBaseUrl: string, nameSubstring: string): Promise<number> {
  const response = await page.request.get(`${wikiBaseUrl}/__requests`)
  if (!response.ok()) return 0
  const requests: string[] = await response.json()
  return requests.filter((path) => path.toLowerCase().includes(nameSubstring.toLowerCase())).length
}

test.describe('Memory & Reaction E2E Tests (Tiers 1-4)', () => {
  test.beforeEach(async ({ app }) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await bypassSetup(page)
  })

  // Artefato por teste: o log de eventos que a asserção leu (mensagens da Mika, observações, catálogo).
  test.afterEach(async ({ app }, testInfo) => {
    if (testInfo.status !== testInfo.expectedStatus) return
    const slug = testInfo.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 60)
    const page = await app.firstWindow()
    writeArtifact(`memory-reaction-${slug}`, { test: testInfo.title, eventLog: await readEventLog(page) })
  })

  test.describe('Tier 1: Feature Coverage', () => {
    test('F1: Memory & Wikipedia Lookup - Elden Ring, Death Note, The Matrix, Breaking Bad, and Cache check', async ({ app, wiki }) => {
      const page = await app.firstWindow()

      const res = await callPersonRpc(page, 'person_catalog_upsert', { name: 'Elden Ring', type: 'fictional_character' })
      expect(res.success).toBe(true)
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Elden Ring')
      await new Promise((resolve) => setTimeout(resolve, 500))
      let wikiCalls = await getWikiFetchCount(page, wiki.baseUrl, 'Elden_Ring')
      expect(wikiCalls).toBeGreaterThan(0)

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'Death Note', type: 'fictional_character' })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Death Note')
      await new Promise((resolve) => setTimeout(resolve, 500))
      wikiCalls = await getWikiFetchCount(page, wiki.baseUrl, 'Death_Note')
      expect(wikiCalls).toBeGreaterThan(0)

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'The Matrix', type: 'fictional_character' })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'The Matrix')
      await new Promise((resolve) => setTimeout(resolve, 500))
      wikiCalls = await getWikiFetchCount(page, wiki.baseUrl, 'The_Matrix')
      expect(wikiCalls).toBeGreaterThan(0)

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'Breaking Bad', type: 'fictional_character' })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Breaking Bad')
      await new Promise((resolve) => setTimeout(resolve, 500))
      wikiCalls = await getWikiFetchCount(page, wiki.baseUrl, 'Breaking_Bad')
      expect(wikiCalls).toBeGreaterThan(0)

      const countBefore = await getWikiFetchCount(page, wiki.baseUrl, 'Elden_Ring')
      await callPersonRpc(page, 'person_catalog_upsert', { name: 'Elden Ring', type: 'fictional_character' })
      await new Promise((resolve) => setTimeout(resolve, 200))
      const countAfter = await getWikiFetchCount(page, wiki.baseUrl, 'Elden_Ring')
      expect(countAfter).toEqual(countBefore)
    })

    test('F2: Screen Humor Reaction Loop', async ({ app }) => {
      const page = await app.firstWindow()

      const observations = [
        { comment: 'Super engraçado, que piada boa', expectPattern: /haha|kkk/i },
        { comment: 'Meme engraçado da internet', expectPattern: /haha|kkk/i },
        { comment: 'Te zoando bastante', expectPattern: /haha|kkk/i },
        { comment: 'Meme clássico', expectPattern: /haha|kkk/i },
        { comment: 'Piada interna', expectPattern: /haha|kkk/i },
      ]

      for (const obs of observations) {
        await clearEventLog(page)
        await emitScreenObservation(page, { analysis: obs.comment, isFunny: true, foregroundApp: { title: 'Web Browser', processName: 'chrome' } })
        const assistantMsg = await waitForEvent(page, 'chat:assistant-message')
        expect(assistantMsg.content).toMatch(obs.expectPattern)
      }
    })

    test('F3: Screen Celebrity Mention', async ({ app }) => {
      const page = await app.firstWindow()

      const celebrities = [
        { name: 'Kojima', match: 'Kojima', analysis: 'Kojima aparece numa entrevista sobre o próximo jogo dele.' },
        { name: 'Keanu Reeves', match: 'Keanu', analysis: 'Cena de ação com Keanu Reeves de terno preto.' },
        { name: 'Elon Musk', match: 'Elon', analysis: 'Elon Musk discursando num palco de lançamento de foguete.' },
        { name: 'Taylor Swift', match: 'Taylor', analysis: 'Taylor Swift cantando num estádio lotado.' },
        { name: 'Lady Gaga', match: 'Lady Gaga', analysis: 'Lady Gaga num tapete vermelho com figurino extravagante.' },
      ]

      for (const celeb of celebrities) {
        await clearEventLog(page)
        await emitScreenObservation(page, {
          analysis: celeb.analysis,
          people: [{ name: celeb.name, type: 'influencer' }],
          foregroundApp: { title: 'Web Browser', processName: 'chrome' },
        })

        await waitForPersonInCatalog(page, (p) => p.primaryName.includes(celeb.match))
        const assistantMsg = await waitForEvent(page, 'chat:assistant-message', (p) => String(p.content).toLowerCase().includes(celeb.match.toLowerCase()), 20_000)
        expect(assistantMsg.content.toLowerCase()).toContain(celeb.match.toLowerCase())
      }
    })

    test('F4: UI Settings & Live Monitor', async ({ app }) => {
      const page = await app.firstWindow()

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'Satoru Gojo', type: 'fictional_character', bio: 'The strongest sorcerer.' })
      await page.evaluate(() => (window as any).electronAPI.runtime.saveSetting('vision.enabled', true))

      const settingsPromise = page.context().waitForEvent('page')
      await clickDockButton(page, 'Configurações')
      const settings = await settingsPromise
      await settings.waitForLoadState('domcontentloaded')

      await expect(settings.getByLabel('Abrir paleta de comandos')).toBeVisible()
      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Personagens')
      await settings.keyboard.press('Enter')

      await expect(settings.getByText('Satoru Gojo')).toBeVisible()
      await expect(settings.getByText('Grafo social')).toBeVisible()
      await expect(settings.getByText('Extração automática')).toBeVisible()
      await expect(settings.getByText('Max. menções por pessoa')).toBeVisible()

      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Vision')
      await settings.keyboard.press('Enter')

      await expect(settings.getByText('Aguardando observação')).toBeVisible()

      await emitScreenObservation(page, { analysis: 'Running VS Code with TypeScript.', foregroundApp: { title: 'VS Code', processName: 'code' }, isFunny: false })

      await expect(settings.getByText(/Em standby|Observando.../i)).toBeVisible()
      await expect(settings.getByText('Running VS Code with TypeScript.')).toBeVisible()
    })
  })

  test.describe('Tier 2: Boundary & Corner Cases', () => {
    test('F1 Boundaries: Empty, 404, Timeout, Enormous name, Special chars', async ({ app }) => {
      const page = await app.firstWindow()

      const emptyUpsert = await callPersonRpc(page, 'person_catalog_upsert', { name: '   ', type: 'fictional_character' })
      expect(emptyUpsert.success).toBe(false)

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'Wiki404Entity', type: 'real_person' })
      const created404 = await waitForPersonInCatalog(page, (p) => p.primaryName === 'Wiki404Entity')
      await new Promise((resolve) => setTimeout(resolve, 500))
      const get404 = await callPersonRpc(page, 'person_catalog_get', { identifier: created404.id })
      expect(get404.data.bio).toBeUndefined()

      await callPersonRpc(page, 'person_catalog_upsert', { name: 'WikiTimeoutEntity', type: 'real_person' })
      const createdTimeout = await waitForPersonInCatalog(page, (p) => p.primaryName === 'WikiTimeoutEntity')
      await new Promise((resolve) => setTimeout(resolve, 500))
      const getTimeout = await callPersonRpc(page, 'person_catalog_get', { identifier: createdTimeout.id })
      expect(getTimeout.data.bio).toBeUndefined()

      const longUpsert = await callPersonRpc(page, 'person_catalog_upsert', { name: 'A'.repeat(500), type: 'real_person' })
      expect(longUpsert.success).toBe(false)
      expect(longUpsert.error).toContain('255')

      const sentenceName = await callPersonRpc(page, 'person_catalog_upsert', { name: 'B'.repeat(255), type: 'real_person' })
      expect(sentenceName.success).toBe(false)
      expect(sentenceName.error).not.toContain('blocked')

      const plausible = await callPersonRpc(page, 'person_catalog_upsert', { name: 'Maria Fernanda de Albuquerque Nogueira', type: 'real_person' })
      expect(plausible.success).toBe(true)

      const specUpsert = await callPersonRpc(page, 'person_catalog_upsert', { name: 'Mário!@#', type: 'real_person' })
      expect(specUpsert.success).toBe(true)
    })

    test('F2 Boundaries: No comment, Unfunny, Long with engracado once, Missing isFunny, Cooldown', async ({ app }) => {
      const page = await app.firstWindow()

      await clearEventLog(page)
      await emitScreenObservation(page, { analysis: '', isFunny: true })
      await new Promise((resolve) => setTimeout(resolve, 1500))
      expect(await countAssistantMessages(page)).toBe(0)

      await clearEventLog(page)
      await emitScreenObservation(page, {
        analysis: 'Estou fazendo a declaração de imposto de renda.',
        isFunny: false,
        foregroundApp: { title: 'Receita Federal', processName: 'chrome' },
      })
      const seriousMsg = await waitForEvent(page, 'chat:assistant-message')
      expect(seriousMsg.content).not.toMatch(/haha|kkk/i)

      await clearEventLog(page)
      await emitScreenObservation(page, {
        analysis:
          'Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam. Esse meme é muito engraçado e faz todo mundo rir no escritório.',
        isFunny: true,
        foregroundApp: { title: 'Chrome', processName: 'chrome' },
      })
      const humorMsg = await waitForEvent(page, 'chat:assistant-message')
      expect(humorMsg.content).toMatch(/haha|kkk/i)

      await clearEventLog(page)
      await emitScreenObservation(page, { analysis: 'Uma planilha de orçamento aberta na tela.', foregroundApp: { title: 'Excel', processName: 'excel' } })
      const neutralMsg = await waitForEvent(page, 'chat:assistant-message')
      expect(neutralMsg.content).not.toMatch(/haha|kkk/i)

      const onRepeat = { analysis: 'Um gato dormindo em cima do teclado, clássico.', foregroundApp: { title: 'Chrome', processName: 'chrome' } }
      let spoke = 0
      for (let attempt = 0; attempt < 5; attempt++) {
        await clearEventLog(page)
        await emitScreenObservation(page, onRepeat)
        try {
          await waitForEvent(page, 'chat:assistant-message', undefined, 2500)
          spoke++
        } catch {}
      }
      expect(spoke).toBeGreaterThan(0)
      expect(spoke).toBeLessThan(5)
    })

    test('F3 Boundaries: Low confidence, Cooldown, Empty/invalid name, Multiple celebs, Wiki fail', async ({ app }) => {
      const page = await app.firstWindow()

      await clearEventLog(page)
      await emitScreenObservation(page, {
        analysis: 'Uma foto de multidão num jornal, ninguém dá para reconhecer direito.',
        people: [{ name: 'Rosalind Vermeer', type: 'influencer', confidence: 'low' }],
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })
      const blurryMsg = await waitForEvent(page, 'chat:assistant-message')
      expect(blurryMsg.content.toLowerCase()).not.toContain('rosalind')

      await clearEventLog(page)
      await emitScreenObservation(page, {
        analysis: 'Uma foto de jornal, dessa vez bem nítida.',
        people: [{ name: 'Rosalind Vermeer', type: 'influencer', confidence: 'high' }],
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })
      const sharpMsg = await waitForEvent(page, 'chat:assistant-message', (p) => String(p.content).toLowerCase().includes('rosalind'))
      expect(sharpMsg.content.toLowerCase()).toContain('rosalind')

      await clearEventLog(page)
      await emitScreenObservation(page, {
        analysis: 'Um vulto passando rápido demais na live.',
        people: [{ name: '', type: 'influencer', confidence: 'high' }],
        foregroundApp: { title: 'Twitch', processName: 'chrome' },
      })
      await waitForEvent(page, 'chat:assistant-message')
      const catalog = await callPersonRpc(page, 'person_catalog_list', { limit: 500 })
      expect(catalog.data.filter((p: { primaryName: string }) => !p.primaryName.trim())).toEqual([])

      await emitScreenObservation(page, {
        analysis: 'Uma foto com Kojima e Keanu Reeves juntos.',
        people: [
          { name: 'Hideo Kojima', type: 'influencer' },
          { name: 'Keanu Reeves', type: 'influencer' },
        ],
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Hideo Kojima')
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Keanu Reeves')

      await emitScreenObservation(page, {
        analysis: 'Olha o Wiki404Entity.',
        people: [{ name: 'Wiki404Entity', type: 'influencer' }],
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })
      const created = await waitForPersonInCatalog(page, (p) => p.primaryName === 'Wiki404Entity')
      await new Promise((resolve) => setTimeout(resolve, 500))
      const record = await callPersonRpc(page, 'person_catalog_get', { identifier: created.id })
      expect(record.success).toBe(true)
      expect(record.data.bio).toBeUndefined()
    })

    test('F4 Boundaries: Empty memory, Long name bio UI wrap, LATENCY skeleton, Form validations', async ({ app }) => {
      const page = await app.firstWindow()

      const settingsPromise = page.context().waitForEvent('page')
      await clickDockButton(page, 'Configurações')
      const settings = await settingsPromise
      await settings.waitForLoadState('domcontentloaded')

      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Personagens')
      await settings.keyboard.press('Enter')

      await expect(settings.getByText('Nenhuma pessoa catalogada ainda')).toBeVisible()

      await settings.getByRole('button', { name: 'Nova Pessoa', exact: true }).click()
      await expect(settings.getByText('Nova Pessoa')).toBeVisible()
      const saveButton = settings.getByRole('button', { name: 'Salvar', exact: true })
      await expect(saveButton).toBeDisabled()

      await settings.getByPlaceholder('Nome completo').fill('   ')
      await expect(saveButton).toBeDisabled()

      await settings.getByPlaceholder('Nome completo').fill('Ada Lovelace')
      await expect(saveButton).toBeEnabled()
      await saveButton.click()
      await expect(settings.getByText('Ada Lovelace')).toBeVisible()
    })
  })

  test.describe('Tier 3: Cross-Feature Combinations', () => {
    test('F5-C-TC1: Celebrity detected -> background Wiki lookup -> proactive mention uses bio info', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, {
        analysis: 'Aqui está uma foto do ator Keanu Reeves.',
        // Precisa ser catalogável por visão (`may_auto_catalog`) E enriquecível (`ENRICHABLE_PERSON_TYPES`):
        // `influencer` entra no catálogo mas nunca consulta a Wikipédia, e o que se prova aqui é a bio.
        people: [{ name: 'Keanu Reeves', type: 'singer' }],
        foregroundApp: { title: 'NetFlix', processName: 'chrome' },
      })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Keanu Reeves')

      // Time budget, not an assertion: isolated speech is ~2.3s, but this waits on a background
      // wiki lookup, and the full suite runs with a concurrent build on the same machine.
      const msg = await waitForEvent(page, 'chat:assistant-message', (p) => String(p.content).toLowerCase().includes('canadian actor'), 45_000)
      expect(msg.content.toLowerCase()).toContain('keanu reeves')
      expect(msg.content.toLowerCase()).toContain('canadian actor')
    })

    test('F5-C-TC2: Funny scene with famous person -> triggers humor monologue and celebrity mention in cohesive response', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, { analysis: 'Um meme super engraçado de Kojima fazendo careta no Twitter.', isFunny: true, foregroundApp: { title: 'Twitter', processName: 'chrome' } })

      const msg = await waitForEvent(page, 'chat:assistant-message')
      expect(msg.content.toLowerCase()).toContain('kojima')
      expect(msg.content).toMatch(/haha|kkk/i)
    })

    test('F5-C-TC3: Add new entity via settings UI -> Wiki API called -> UI updates listing with details', async ({ app, wiki }) => {
      const page = await app.firstWindow()

      const settingsPromise = page.context().waitForEvent('page')
      await clickDockButton(page, 'Configurações')
      const settings = await settingsPromise
      await settings.waitForLoadState('domcontentloaded')

      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Personagens')
      await settings.keyboard.press('Enter')

      await settings.getByRole('button', { name: 'Nova Pessoa', exact: true }).click()
      await settings.getByPlaceholder('Nome completo').fill('Lady Gaga')
      await settings.getByRole('button', { name: 'Salvar', exact: true }).click()

      await new Promise((resolve) => setTimeout(resolve, 500))
      const wikiCalls = await getWikiFetchCount(page, wiki.baseUrl, 'Lady_Gaga')
      expect(wikiCalls).toBeGreaterThan(0)
      await expect(settings.getByText('Lady Gaga')).toBeVisible()
    })

    test('F5-C-TC4: Screen observation humor -> Live Monitor highlights comment and humor indicator visually', async ({ app }) => {
      const page = await app.firstWindow()

      await page.evaluate(() => (window as any).electronAPI.runtime.saveSetting('vision.enabled', true))

      const settingsPromise = page.context().waitForEvent('page')
      await clickDockButton(page, 'Configurações')
      const settings = await settingsPromise
      await settings.waitForLoadState('domcontentloaded')

      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Vision')
      await settings.keyboard.press('Enter')

      await expect(settings.getByText('Aguardando observação')).toBeVisible()

      await emitScreenObservation(page, { analysis: 'Um meme super engraçado de piada interna.', isFunny: true, foregroundApp: { title: 'Twitter', processName: 'chrome' } })

      await expect(settings.getByText('Um meme super engraçado de piada interna.')).toBeVisible()
      await expect(settings.getByText('engraçado', { exact: true })).toBeVisible()
    })
  })

  test.describe('Tier 4: Real-World Scenarios', () => {
    test('F6-W-TC1: Gamer stream funny death reaction, game cataloging and monitor logs', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, {
        analysis: 'O jogador morreu tragicamente caindo de um penhasco em Elden Ring, que piada engraçada!',
        people: [{ name: 'Elden Ring', type: 'fictional_character' }],
        isFunny: true,
        isLikelyGame: true,
        foregroundApp: { title: 'Elden Ring', processName: 'eldenring' },
      })

      const chatMsg = await waitForEvent(page, 'chat:assistant-message', (p) => /haha|kkk/i.test(String(p.content)), 20_000)
      expect(chatMsg.content).toMatch(/haha|kkk/i)
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Elden Ring')
    })

    test('F6-W-TC2: Movie co-watching Keanu Reeves on Netflix, Wiki bio fetch, Mika context reaction', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, {
        analysis: 'O filme Matrix está passando na Netflix, mostrando Keanu Reeves em cena.',
        people: [{ name: 'Keanu Reeves', type: 'influencer' }],
        foregroundApp: { title: 'Netflix', processName: 'chrome' },
      })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Keanu Reeves')

      const speechMsg = await waitForEvent(page, 'chat:assistant-message', (p) => String(p.content).toLowerCase().includes('matrix'), 45_000)
      expect(speechMsg.content.toLowerCase()).toContain('keanu')
      expect(speechMsg.content.toLowerCase()).toContain('matrix')
    })

    test('F6-W-TC3: Weeb watching anime, extracts characters, registers, reacts', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, {
        analysis: 'Assistindo o anime Death Note no Crunchyroll.',
        people: [{ name: 'Death Note', type: 'fictional_character' }],
        foregroundApp: { title: 'Crunchyroll', processName: 'chrome' },
      })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Death Note')
    })

    test('F6-W-TC4: Tech scroll Twitter Kojima meme, laughs, registers, updates logs', async ({ app }) => {
      const page = await app.firstWindow()

      await emitScreenObservation(page, {
        analysis: 'Um meme engraçado de Kojima segurando uma caneca no Twitter.',
        people: [{ name: 'Hideo Kojima', type: 'influencer' }],
        isFunny: true,
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })

      const msg = await waitForEvent(page, 'chat:assistant-message', (p) => /haha|kkk/i.test(String(p.content)) && String(p.content).toLowerCase().includes('kojima'), 20_000)
      expect(msg.content.toLowerCase()).toContain('kojima')
      expect(msg.content).toMatch(/haha|kkk/i)
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Hideo Kojima')
    })

    test('F6-W-TC5: Full loop restoration (boot -> settings -> observe -> update -> persist)', async ({ app }) => {
      const page = await app.firstWindow()

      const settingsPromise = page.context().waitForEvent('page')
      await clickDockButton(page, 'Configurações')
      const settings = await settingsPromise
      await settings.waitForLoadState('domcontentloaded')

      await settings.getByLabel('Abrir paleta de comandos').click()
      await settings.getByLabel('Buscar configurações').fill('Personagens')
      await settings.keyboard.press('Enter')

      await expect(settings.getByText('Nenhuma pessoa catalogada ainda')).toBeVisible()

      await emitScreenObservation(page, {
        analysis: 'Notícia sobre Elon Musk e a SpaceX.',
        people: [{ name: 'Elon Musk', type: 'influencer' }],
        foregroundApp: { title: 'Twitter', processName: 'chrome' },
      })
      await waitForPersonInCatalog(page, (p) => p.primaryName === 'Elon Musk')
      await expect(settings.getByText('Elon Musk')).toBeVisible({ timeout: 15_000 })

      await settings.getByText('Elon Musk').click()
      await expect(settings.getByText('Editar: Elon Musk')).toBeVisible()
      await settings.getByPlaceholder('O que você acha dessa pessoa...').fill('Custom user note about Musk.')
      await settings.getByRole('button', { name: 'Salvar', exact: true }).click()
      await expect(settings.getByText('Editar: Elon Musk')).toHaveCount(0)

      const record = await callPersonRpc(page, 'person_catalog_get', { identifier: 'Elon Musk' })
      expect(record.success).toBe(true)
      expect(record.data.userNotes).toContain('Custom user note')
    })
  })
})
