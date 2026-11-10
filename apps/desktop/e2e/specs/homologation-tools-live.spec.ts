import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

const CASES = JSON.parse(
  readFileSync(join(process.cwd(), '..', '..', 'scripts', 'homologation', 'homologate-tools.cases.json'), 'utf8'),
) as Record<string, unknown>

// Policy: every case from homologate-tools.cases.json also runs here against the live core. Only
// SKIP_LIVE entries are genuinely unsafe on a real Electron app (same bar as `_unsafe` in cases.json),
// each with its reason on record — desktop:launch (invalid target, rejected before shellAdapter runs)
// and play_minigame/match_effect (execute() only returns data; window/overlay effects only happen
// through the chat tool-loop, which direct executeTool calls don't traverse) need no skip.
const SKIP_LIVE = new Set<string>([])

// Checks real CONTENT, not just shape — the same defect class that hit web:fetch (success:true with
// plausible but wrong/empty data) survives a hollow-check when the field exists but the value is wrong.
const CONTENT_CHECKS: Record<string, (data: unknown) => string | null> = {
  // Default avatar became VRM; the kind×extension pair must match whatever model is actually loaded.
  'avatar:inspect_model': (data) => {
    const d = data as { modelPath?: string; motions?: unknown[]; kind?: string }
    const esperado = d.modelPath?.toLowerCase().endsWith('.vrm')
      ? 'vrm'
      : d.modelPath?.toLowerCase().endsWith('.model3.json')
        ? 'live2d'
        : null
    if (!esperado) {
      return `modelPath deveria terminar em .vrm ou .model3.json, veio ${JSON.stringify(d.modelPath)}`
    }
    if (d.kind !== esperado) {
      return `kind deveria ser "${esperado}" para ${JSON.stringify(d.modelPath)}, veio ${JSON.stringify(d.kind)}`
    }
    if (!Array.isArray(d.motions) || d.motions.length === 0) {
      return `motions deveria ser uma lista não vazia, veio ${JSON.stringify(d.motions)}`
    }
    return null
  },
  'avatar:affinity_delta': (data) => {
    const d = data as { previous?: number; delta?: number; next?: number }
    if (d.delta !== 0) return `delta deveria ser 0 (caso pede delta:0), veio ${d.delta}`
    if (d.next !== d.previous) return `next deveria igualar previous com delta 0: ${d.previous} != ${d.next}`
    return null
  },
  'game_knowledge:ask': (data) => {
    const d = data as { game?: string; topic?: string; guide?: string }
    if (d.game !== 'Elden Ring') return `game deveria ser "Elden Ring", veio ${JSON.stringify(d.game)}`
    if (d.topic !== 'quem e Melina?') return `topic deveria ecoar o foco pedido, veio ${JSON.stringify(d.topic)}`
    if (typeof d.guide !== 'string' || d.guide.trim().length === 0) return 'guide veio vazio'
    return null
  },
  'game_knowledge:memorize_location': (data) => {
    const d = data as { location?: { locationName?: string; gameName?: string } }
    if (d.location?.locationName !== 'Limgrave')
      return `locationName deveria ser "Limgrave", veio ${JSON.stringify(d.location?.locationName)}`
    if (d.location?.gameName !== 'Elden Ring')
      return `gameName deveria ser "Elden Ring", veio ${JSON.stringify(d.location?.gameName)}`
    return null
  },
  'mood:update': (data) => {
    const d = data as { valence?: number; arousal?: number }
    if (d.valence !== 0.1 || d.arousal !== 0.1)
      return `valence/arousal deveriam ser 0.1/0.1, vieram ${d.valence}/${d.arousal}`
    return null
  },
  'person_catalog:upsert': (data) => {
    const d = data as { primaryName?: string }
    if (d.primaryName !== 'Homologacao')
      return `primaryName deveria ser "Homologacao", veio ${JSON.stringify(d.primaryName)}`
    return null
  },
  'person_catalog:get': (data) => {
    const d = data as { primaryName?: string }
    if (d.primaryName !== 'Homologacao')
      return `primaryName deveria ser "Homologacao", veio ${JSON.stringify(d.primaryName)}`
    return null
  },
  'person_catalog:list': (data) => {
    const list = data as Array<{ primaryName?: string }>
    if (!Array.isArray(list) || !list.some((p) => p.primaryName === 'Homologacao')) {
      return `lista deveria conter "Homologacao", veio ${JSON.stringify(list).slice(0, 200)}`
    }
    return null
  },
  'person_catalog:search': (data) => {
    const list = data as Array<{ primaryName?: string }>
    if (!Array.isArray(list) || !list.some((p) => p.primaryName === 'Homologacao')) {
      return `busca por "Homologacao" deveria achar o próprio perfil, veio ${JSON.stringify(list).slice(0, 200)}`
    }
    return null
  },
  'person_catalog:add_fact': (data) => {
    const d = data as { fact?: string }
    if (d.fact !== 'existe so para o harness')
      return `fact deveria ecoar o que foi pedido, veio ${JSON.stringify(d.fact)}`
    return null
  },
  'user_profile:learn': (data) => {
    const d = data as { fact?: string; category?: string }
    if (d.fact !== 'homologacao do harness' || d.category !== 'other') {
      return `fact/category deveriam ecoar o pedido, vieram ${JSON.stringify(d.fact)}/${JSON.stringify(d.category)}`
    }
    return null
  },
  'user_profile:query': (data) => {
    const d = data as { query?: string }
    if (d.query !== 'quem sou eu?') return `query deveria ecoar a pergunta, veio ${JSON.stringify(d.query)}`
    return null
  },
  'memory_console:add_fact': (data) => {
    const d = data as { fact?: string }
    if (d.fact !== 'fato de homologacao') return `fact deveria ecoar o pedido, veio ${JSON.stringify(d.fact)}`
    return null
  },
  'reminder:schedule': (data) => {
    const d = data as { subject?: string }
    if (d.subject !== 'homologacao') return `subject deveria ecoar o pedido, veio ${JSON.stringify(d.subject)}`
    return null
  },
  'content:daily_recap': (data) => {
    const d = data as { recap?: unknown }
    if (typeof d.recap !== 'string' || d.recap.trim().length === 0) return 'recap deveria ser texto não vazio'
    return null
  },
}

test('homologação: as tools respondem com o núcleo de pé', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  await page.evaluate(() =>
    (
      window as unknown as { electronAPI: { runtime: { saveSetting: (k: string, v: unknown) => Promise<unknown> } } }
    ).electronAPI.runtime.saveSetting('brain.approveRiskyTools', false),
  )
  const registered = await page.evaluate(async () => {
    const api = (
      window as unknown as { electronAPI: { runtime: { getTools: () => Promise<{ tools: Array<{ name: string }> }> } } }
    ).electronAPI
    return (await api.runtime.getTools()).tools.map((t) => t.name)
  })

  const plan: Array<{ tool: string; label: string; args: Record<string, unknown> }> = []

  const skipped: string[] = []
  for (const [tool, raw] of Object.entries(CASES)) {
    if (tool.startsWith('_')) continue
    const cases = Array.isArray(raw) ? raw : [raw]
    for (const args of cases as Array<Record<string, unknown>>) {
      const action = typeof args.action === 'string' ? args.action : ''
      const label = action ? `${tool}:${action}` : tool
      if (SKIP_LIVE.has(label)) {
        skipped.push(label)
        continue
      }
      plan.push({ tool, label, args })
    }
  }
  const staleSkips = [...SKIP_LIVE].filter((label) => !skipped.includes(label))

  const OFF_BY_DEFAULT = new Set(['discord', 'discord_voice'])
  const absent = [...new Set(plan.map((p) => p.tool))].filter((t) => !registered.includes(t))
  const missing = absent.filter((t) => !OFF_BY_DEFAULT.has(t))
  for (const tool of absent.filter((t) => OFF_BY_DEFAULT.has(t))) {
    console.log(`⏭️  ${tool} — feature desligada por default num perfil novo`)
  }

  const results = await page.evaluate(
    async (calls: Array<{ tool: string; label: string; args: Record<string, unknown> }>) => {
      const api = (
        window as unknown as {
          electronAPI: { runtime: { executeTool: (n: string, a: Record<string, unknown>) => Promise<unknown> } }
        }
      ).electronAPI
      const out: Array<{
        label: string
        ok: boolean
        success: unknown
        hollow: boolean
        detail: string
        data: unknown
      }> = []
      for (const call of calls) {
        try {
          const envelope = (await api.runtime.executeTool(call.tool, call.args)) as {
            success?: unknown
            data?: unknown
            error?: unknown
          }
          const shaped = typeof envelope === 'object' && envelope !== null && typeof envelope.success === 'boolean'

          const d = envelope?.data
          const hollow =
            envelope?.success === true &&
            (d === undefined ||
              d === null ||
              d === '' ||
              (Array.isArray(d) && d.length === 0) ||
              (typeof d === 'object' && !Array.isArray(d) && Object.keys(d as object).length === 0))
          out.push({
            label: call.label,
            ok: shaped,
            success: envelope?.success,
            hollow,
            data: envelope?.success === true ? d : undefined,

            detail: shaped
              ? String(
                  envelope.error ??
                    (hollow
                      ? 'success:true com data VAZIO'
                      : Array.isArray(d)
                        ? `ok · array(${d.length})`
                        : typeof d === 'object' && d !== null
                          ? `ok · ${Object.entries(d as Record<string, unknown>)
                              .map(([k, v]) => {
                                if (Array.isArray(v)) return `${k}(${v.length})`
                                if (v === null) return `${k}=null`
                                if (v === '') return `${k}=''`

                                if (v instanceof Date) return k
                                if (typeof v === 'object' && Object.keys(v as object).length === 0) return `${k}{}`
                                return k
                              })
                              .slice(0, 6)
                              .join(', ')}`
                          : 'ok'),
                )
              : `envelope inválido: ${JSON.stringify(envelope)?.slice(0, 120)}`,
          })
        } catch (err) {
          out.push({
            label: call.label,
            ok: false,
            success: undefined,
            hollow: false,
            data: undefined,
            detail: `throw: ${String(err)}`,
          })
        }
      }
      return out
    },
    plan,
  )

  const typed = results as Array<{
    label: string
    ok: boolean
    success: unknown
    hollow: boolean
    detail: string
    data: unknown
  }>
  const succeeded = typed.filter((r) => r.success === true)
  for (const r of typed) {
    console.log(`${r.ok ? (r.success === true ? '✅' : '⚠️ ') : '❌'} ${r.label} — ${String(r.detail).slice(0, 110)}`)
  }
  const hollow = typed.filter((r) => r.hollow)
  console.log(
    `\n${typed.length} ações executadas com o núcleo de pé · ${succeeded.length} responderam success:true · ${skipped.length} puladas (${skipped.join(', ')})`,
  )
  if (staleSkips.length > 0) {
    console.log(`⚠️  em SKIP_LIVE e sem caso correspondente: ${staleSkips.join(', ')}`)
  }
  if (hollow.length > 0)
    console.log(`⚠️  ${hollow.length} com success:true e data VAZIO: ${hollow.map((r) => r.label).join(', ')}`)

  const contentFailures: string[] = []
  for (const r of typed) {
    if (r.success !== true) continue
    const check = CONTENT_CHECKS[r.label]
    if (!check) continue
    const problem = check(r.data)
    if (problem) contentFailures.push(`${r.label}: ${problem}`)
  }
  if (contentFailures.length > 0)
    console.log(`⚠️  ${contentFailures.length} com conteúdo incorreto: ${contentFailures.join(' | ')}`)

  expect(missing, 'tool com caso declarado que não está registrada no app').toEqual([])
  expect(typed.length).toBeGreaterThan(60)
  expect(typed.filter((r) => !r.ok).map((r) => `${r.label}: ${r.detail}`)).toEqual([])
  expect(contentFailures, 'success:true com CONTEÚDO incorreto, não só forma').toEqual([])
  writeArtifact('homologation-tools-live', {
    executed: typed.length,
    succeeded: succeeded.length,
    skipped,
    hollow: hollow.map((r) => r.label),
    results: typed.map(({ label, ok, success, hollow: isHollow, detail }) => ({ label, ok, success, hollow: isHollow, detail })),
  })
})
