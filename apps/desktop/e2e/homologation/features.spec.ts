import type { Page } from '@playwright/test'
import { expect, test } from './fixtures/homolog-app'
import { createRecorder, looksLikeMissingCredential, summarize } from './fixtures/recorder'

const recorder = createRecorder('Homologação funcional — features do roster', 'features')
const { record } = recorder

async function testPlugin(window: Page, pluginId: string): Promise<{ ok: boolean; message?: string }> {
  return await window.evaluate(async (id) => {
    const runtime = (
      window as unknown as {
        electronAPI: {
          runtime: { testPlugin: (p: { pluginId: string }) => Promise<{ ok: boolean; message?: string }> }
        }
      }
    ).electronAPI.runtime
    try {
      return await runtime.testPlugin({ pluginId: id })
    } catch (error) {
      return { ok: false, message: error instanceof Error ? error.message : String(error) }
    }
  }, pluginId)
}

test.describe.configure({ mode: 'serial' })

test('features do roster: tools não destrutivas', async ({ window, runTool }) => {
  {
    const expression = await runTool('avatar', { action: 'expression', expression: 'happy', durationMs: 800 })
    record(
      'character',
      'avatar(expression=happy)',
      expression.success === false ? 'falha' : 'ok',
      summarize(expression),
    )
    const affinity = await runTool('avatar', { action: 'affinity_delta', delta: 2, reason: 'homologação' })
    record('character', 'avatar(affinity_delta=+2)', affinity.success === false ? 'falha' : 'ok', summarize(affinity))
  }

  {
    const learn = await runTool('user_profile', { action: 'learn', fact: 'Elden Ring', category: 'hobby' })
    record('user', 'user(learn/hobby)', learn.success === false ? 'falha' : 'ok', summarize(learn))
    const query = await runTool('user_profile', { action: 'query', query: 'Quais são os hobbies dele?' })
    const remembered = JSON.stringify(query.data ?? '')
      .toLowerCase()
      .includes('elden')
    record(
      'user',
      'user(query) lê de volta o que user(learn) gravou',
      query.success === false ? 'falha' : remembered ? 'ok' : 'falha',
      remembered ? summarize(query) : `não retornou o fato gravado — ${summarize(query)}`,
    )
  }

  {
    const before = await runTool('mood', { action: 'get' })
    const update = await runTool('mood', { action: 'update', valence: 0.6, arousal: 0.4 })
    const after = await runTool('mood', { action: 'get' })
    const moved = JSON.stringify(before.data) !== JSON.stringify(after.data)
    record(
      'mood',
      'mood(update) muda o estado lido por mood(get)',
      update.success === false ? 'falha' : moved ? 'ok' : 'falha',
      moved ? summarize(after) : `estado não mudou: ${summarize(after)}`,
    )
    const decay = await runTool('mood', { action: 'decay', factor: 0.5 })
    record('mood', 'mood(decay, factor=0.5)', decay.success === false ? 'falha' : 'ok', summarize(decay))
    const interaction = await runTool('mood', { action: 'register_interaction', weight: 1, reason: 'homologação' })
    record('mood', 'mood(register_interaction)', interaction.success === false ? 'falha' : 'ok', summarize(interaction))

    const hasAffinity = (after.data as { affinity?: unknown } | undefined)?.affinity !== undefined
    record(
      'mood',
      'mood(get) devolve afinidade junto com o humor',
      hasAffinity ? 'ok' : 'falha',
      hasAffinity ? summarize(after) : `resposta sem o campo affinity — ${summarize(after)}`,
    )
  }

  {
    const library = await runTool('steam', { action: 'library', mode: 'list', limit: 5 })
    const games = (library.data as { games?: unknown[] } | undefined)?.games ?? []
    record(
      'gamer-companion',
      'steam(library/list) pelo cache local',
      library.success === false ? (looksLikeMissingCredential(library) ? 'sem-credencial' : 'falha') : 'ok',
      `${games.length} jogos — ${summarize(library)}`,
    )

    const probes: Array<[string, string, Record<string, unknown>]> = [
      ['game_session(playtime)', 'game_session', { action: 'playtime' }],
      ['game_session(stats)', 'game_session', { action: 'stats' }],
      ['game_session(streak)', 'game_session', { action: 'streak' }],
      ['play_minigame(jokenpo)', 'play_minigame', { game: 'jokenpo' }],
    ]
    for (const [label, name, args] of probes) {
      const result = await runTool(name, args)
      record('gamer-companion', label, result.success === false ? 'falha' : 'ok', summarize(result))
    }
  }

  {
    const upsert = await runTool('person_catalog', {
      action: 'upsert',
      name: 'Homologação Teste',
      type: 'fictional_character',
      bio: 'Perfil criado pela homologação automática.',
      tags: ['homologacao'],
    })
    const id = (upsert.data as { id?: string } | undefined)?.id
    record('person-catalog', 'person_catalog(upsert)', upsert.success === false ? 'falha' : 'ok', summarize(upsert))

    const search = await runTool('person_catalog', { action: 'search', query: 'Homologação', limit: 5 })
    const found = JSON.stringify(search.data ?? '').includes('Homologação Teste')
    record(
      'person-catalog',
      'person_catalog(search) acha o que upsert gravou',
      found ? 'ok' : 'falha',
      found ? summarize(search) : `não achou o perfil recém-criado — ${summarize(search)}`,
    )

    if (id) {
      const guard = await runTool('person_catalog', { action: 'delete', id, confirm: false })
      record(
        'person-catalog',
        'person_catalog(delete) exige confirm:true',
        guard.success === false ? 'ok' : 'falha',
        guard.success === false ? `recusou sem confirmação: ${summarize(guard)}` : 'APAGOU SEM CONFIRMAÇÃO',
      )
      const removed = await runTool('person_catalog', { action: 'delete', id, confirm: true })
      record(
        'person-catalog',
        'person_catalog(delete, confirm)',
        removed.success === false ? 'falha' : 'ok',
        summarize(removed),
      )
    }
  }

  {
    const windows = await runTool('desktop', { action: 'window' })
    record('desktop', 'desktop(window)', windows.success === false ? 'falha' : 'ok', summarize(windows))
    const processes = await runTool('desktop', { action: 'process' })
    record('desktop', 'desktop(process)', processes.success === false ? 'falha' : 'ok', summarize(processes))
  }

  {
    const scheduled = await runTool('reminder', {
      action: 'schedule',
      subject: 'conferir a homologação',
      amount: 90,
      unit: 'minutes',
      recurrence: 'once',
    })
    record('scheduler', 'reminder(schedule +90min)', scheduled.success === false ? 'falha' : 'ok', summarize(scheduled))
    const list = await runTool('reminder', { action: 'list' })
    const listed = JSON.stringify(list.data ?? '').includes('homologação')
    record(
      'scheduler',
      'reminder(list) mostra o lembrete criado',
      listed ? 'ok' : 'falha',
      listed ? summarize(list) : `lembrete não apareceu — ${summarize(list)}`,
    )
    const id = (scheduled.data as { id?: string } | undefined)?.id
    if (id) {
      const cancelled = await runTool('reminder', { action: 'cancel', id })
      record('scheduler', 'reminder(cancel)', cancelled.success === false ? 'falha' : 'ok', summarize(cancelled))
    }
  }

  {
    const mcp = await testPlugin(window, 'mika.mcp')
    const message = mcp.message ?? ''
    record(
      'mcp',
      'testConnection(mika.mcp) sem servidor configurado',
      mcp.ok ? 'ok' : /no mcp server|nenhum servidor/i.test(message) ? 'sem-credencial' : 'falha',
      message || 'sem mensagem',
    )
  }

  expect(recorder.results.length).toBeGreaterThan(20)
})

test('mika.privacy: clear_history exige confirmação e apaga de verdade', async ({ runTool }) => {
  const guard = await runTool('clear_history', { confirm: false })
  record(
    'privacy',
    'clear_history exige confirm:true',
    guard.success === false ? 'ok' : 'falha',
    guard.success === false ? `recusou: ${summarize(guard)}` : 'APAGOU SEM CONFIRMAÇÃO',
  )

  const wipe = await runTool('clear_history', { confirm: true })
  record('privacy', 'clear_history(confirm)', wipe.success === false ? 'falha' : 'ok', summarize(wipe))

  const profile = await runTool('user_profile', { action: 'query', query: 'Quais são os hobbies dele?' })
  const stillThere = JSON.stringify(profile.data ?? '')
    .toLowerCase()
    .includes('elden')
  record(
    'privacy',
    'perfil do usuário some depois do wipe',
    stillThere ? 'falha' : 'ok',
    stillThere ? `o fato gravado sobreviveu ao wipe — ${summarize(profile)}` : summarize(profile),
  )

  expect(guard.success).toBe(false)
})

test.afterAll(() => recorder.flush())
