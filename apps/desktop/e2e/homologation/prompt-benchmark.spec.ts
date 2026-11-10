import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { expect, HOMOLOG_PROFILE, test } from './fixtures/homolog-app'

// A/B benchmark of the system prompt: prose tool routing duplicates the tool schema and can go, but
// behavior prohibitions aren't in any schema and stay. Verdict is the tool column, not token/latency.

// Variant A is the shipped `prompts/brain/system.md`; B is the same file minus the prose routing
// section, derived at run time so the two never drift apart the way a hand-copied fixture did (the
// old `fixtures/prompts/system.sem-roteamento.md` was a Portuguese Electron-era prompt, so the A/B
// compared two different prompts, not "with vs without routing"). B reaches the app through the
// user override root the Rust loader reads every turn: `<profile>/prompt-overrides/brain/system.md`.
const SHIPPED_SYSTEM = resolve(__dirname, '..', '..', 'prompts', 'brain', 'system.md')
const ROUTING_HEADING = '## Vision, search and PC actions'
const OVERRIDE = join(HOMOLOG_PROFILE, 'prompt-overrides', 'brain', 'system.md')

function withoutRoutingProse(prompt: string): string {
  const start = prompt.indexOf(ROUTING_HEADING)
  expect(start, `${ROUTING_HEADING} sumiu do system.md — atualize o benchmark`).toBeGreaterThan(-1)
  const next = prompt.indexOf('\n## ', start + ROUTING_HEADING.length)
  return prompt.slice(0, start) + (next === -1 ? '' : prompt.slice(next + 1))
}

// n=1 is a coin flip; run variance within the same variant can rival the difference between variants.
const REPEATS = Number(process.env.MIKA_BENCH_REPEATS ?? 3)
const REPORT_DIR = join(__dirname, 'report')

const CASES = [
  { fala: 'O que tem na minha tela agora?', esperado: 'vision' },
  { fala: 'Abre a calculadora pra mim.', esperado: 'desktop' },
  { fala: 'Quem ganhou o último Major de Counter-Strike?', esperado: 'web' },
  { fala: 'Prefiro sushi a pizza, e você?', esperado: null },
  { fala: 'Bom dia, tudo bem com você?', esperado: null },
] as const

type CaseResult = {
  fala: string
  esperado: string | null
  ferramentas: string[]
  acertos: number
  tentativas: number
  promptTokens: number
  completionTokens: number
  ms: number
  resposta: string
}

// usage doesn't come back over IPC (Zod schema strips undeclared fields); only the log has the number.
function lastUsage(): { promptTokens: number; completionTokens: number } {
  try {
    const raw = readFileSync(join(HOMOLOG_PROFILE, 'logs', 'mika-ai.ndjson'), 'utf8')
    const lines = raw.split('\n').filter((line) => line.trim().length > 0)
    for (let i = lines.length - 1; i >= 0; i--) {
      const entry = JSON.parse(lines[i] as string) as { msg?: string; meta?: Record<string, number> }
      if (entry.msg !== 'chat response') continue
      return {
        promptTokens: entry.meta?.promptTokens ?? 0,
        completionTokens: entry.meta?.completionTokens ?? 0,
      }
    }
  } catch {}
  return { promptTokens: 0, completionTokens: 0 }
}

function overrideSystemPrompt(content: string): void {
  if (existsSync(OVERRIDE) && !existsSync(`${OVERRIDE}.bak`)) copyFileSync(OVERRIDE, `${OVERRIDE}.bak`)
  mkdirSync(dirname(OVERRIDE), { recursive: true })
  writeFileSync(OVERRIDE, content, 'utf8')
}

function restoreSystemPrompt(): void {
  if (existsSync(`${OVERRIDE}.bak`)) renameSync(`${OVERRIDE}.bak`, OVERRIDE)
  else rmSync(OVERRIDE, { force: true })
}

// Defaults to local Ollama (no paid benchmark runs); remote via MIKA_BENCH_PROVIDER/MIKA_BENCH_MODEL.
const BENCH_LLM = {
  provider: process.env.MIKA_BENCH_PROVIDER ?? process.env.MIKA_HOMOLOG_PROVIDER ?? 'ollama',
  model: process.env.MIKA_BENCH_MODEL ?? process.env.MIKA_HOMOLOG_MODEL ?? 'qwen3.5:4b',
}

const results = new Map<string, CaseResult[]>()

async function runVariant(
  window: import('@playwright/test').Page,
  label: string,
): Promise<void> {
  await window.evaluate(async ({ provider, model }) => {
    const api = (
      window as unknown as {
        electronAPI: {
          runtime: {
            saveSetting: (p: string, v: unknown) => Promise<unknown>
            onToolCall: (cb: (p: { name: string }) => void) => void
            onToolApprovalRequest: (cb: (r: { toolCallId: string; toolName: string }) => void) => void
            sendToolApprovalResponse: (id: string, approved: boolean, scope?: string) => void
          }
        }
      }
    ).electronAPI.runtime
    await api.saveSetting('llm.provider', provider)
    if (provider === 'openrouter') await api.saveSetting('llm.openrouterModel', model)
    else await api.saveSetting('llm.ollamaModel', model)
    await api.saveSetting('voice.ttsEnabled', false)

    const tools: string[] = []
    ;(window as unknown as { __toolCalls: string[] }).__toolCalls = tools
    api.onToolCall((payload) => tools.push(payload.name))

    // Without answering the approval request, the turn ends without the tool and the benchmark records a false "didn't call".
    api.onToolApprovalRequest((req) => api.sendToolApprovalResponse(req.toolCallId, true, 'once'))
  }, BENCH_LLM)

  const out: CaseResult[] = []
  const erros: string[] = []
  for (const caso of CASES) {
    const chamadas: string[] = []
    let acertos = 0
    let somaPrompt = 0
    let somaCompletion = 0
    let somaMs = 0
    let ultimaResposta = ''

    for (let tentativa = 0; tentativa < REPEATS; tentativa++) {
    // History cleared BEFORE each request, or the prompt grows turn-over-turn into noise.
    await window.evaluate(async () => {
      const api = (window as unknown as { electronAPI: { runtime: { clearChatHistory: () => Promise<unknown> } } })
        .electronAPI.runtime
      await api.clearChatHistory()
    })

    const turno = await window.evaluate(async (fala) => {
      const api = (
        window as unknown as {
          electronAPI: {
            runtime: {
              sendChatMessage: (t: string, a?: unknown, s?: string) => Promise<{ content?: string; role?: string }>
            }
          }
        }
      ).electronAPI.runtime
      ;(window as unknown as { __toolCalls: string[] }).__toolCalls.length = 0
      const started = Date.now()
      const reply = await api.sendChatMessage(fala, undefined, 'text')
      return {
        conteudo: reply.content ?? '',
        // role: 'system' means the turn died — content is the app's error message.
        falhou: reply.role === 'system',
        ms: Date.now() - started,
        ferramentas: (window as unknown as { __toolCalls: string[] }).__toolCalls.slice(),
      }
    }, caso.fala)

      if (turno.falhou) erros.push(turno.conteudo)

      const chamou = turno.ferramentas.some((name) => (caso.esperado ? name.includes(caso.esperado) : false))
      if (caso.esperado === null ? turno.ferramentas.length === 0 : chamou) acertos++
      chamadas.push(turno.ferramentas.join('+') || '—')
      const usage = lastUsage()
      somaPrompt += usage.promptTokens
      somaCompletion += usage.completionTokens
      somaMs += turno.ms
      ultimaResposta = turno.conteudo
    }

    out.push({
      fala: caso.fala,
      esperado: caso.esperado,
      ferramentas: chamadas,
      acertos,
      tentativas: REPEATS,
      promptTokens: Math.round(somaPrompt / REPEATS),
      completionTokens: Math.round(somaCompletion / REPEATS),
      ms: Math.round(somaMs / REPEATS),
      resposta: ultimaResposta,
    })
  }
  results.set(label, out)
  expect(erros, `o turno morreu antes de medir qualquer coisa: ${erros[0] ?? ''}`).toEqual([])
}

test.describe.configure({ mode: 'serial' })

test.describe('A — system.md atual, com roteamento de ferramenta em prosa', () => {
  test.beforeAll(() => {
    restoreSystemPrompt()
  })

  test('cinco pedidos, três deles pedindo ferramenta', async ({ window }) => {
    await runVariant(window, 'A (com prosa)')
  })
})

test.describe('B — system.md sem o roteamento em prosa', () => {
  test.beforeAll(() => {
    overrideSystemPrompt(withoutRoutingProse(readFileSync(SHIPPED_SYSTEM, 'utf8')))
  })

  test.afterAll(() => {
    restoreSystemPrompt()
    writeReport()
  })

  test('os mesmos cinco pedidos', async ({ window }) => {
    await runVariant(window, 'B (sem prosa)')
  })
})

function writeReport(): void {
  mkdirSync(REPORT_DIR, { recursive: true })
  const lines = ['# Benchmark do system prompt: roteamento de ferramenta em prosa', '']

  for (const [label, casos] of results) {
    const acertos = casos.reduce((sum, c) => sum + c.acertos, 0)
    const tentativas = casos.reduce((sum, c) => sum + c.tentativas, 0)
    const prompt = Math.round(casos.reduce((s, c) => s + c.promptTokens, 0) / casos.length)
    const ms = Math.round(casos.reduce((s, c) => s + c.ms, 0) / casos.length)
    lines.push(
      `## ${label}`,
      '',
      `- **Acertou a ferramenta:** ${acertos}/${tentativas}`,
      `- **Prompt médio:** ${prompt} tokens`,
      `- **Turno médio:** ${(ms / 1000).toFixed(1)}s`,
      '',
      '| Pedido | Esperado | Chamou (por tentativa) | Acertos | Prompt | Turno |',
      '| --- | --- | --- | :-: | ---: | ---: |',
      ...casos.map(
        (c) =>
          `| ${c.fala} | ${c.esperado ?? '_(nenhuma)_'} | ${c.ferramentas.join(' · ')} | ${c.acertos}/${c.tentativas} | ${c.promptTokens} | ${(c.ms / 1000).toFixed(1)}s |`,
      ),
      '',
    )
  }

  const a = results.get('A (com prosa)') ?? []
  const b = results.get('B (sem prosa)') ?? []
  if (a.length > 0 && b.length > 0) {
    const media = (list: CaseResult[], key: 'promptTokens' | 'ms') =>
      Math.round(list.reduce((s, c) => s + c[key], 0) / list.length)
    lines.push(
      '## Veredito',
      '',
      `- Ferramenta certa: **A ${a.reduce((s2, c) => s2 + c.acertos, 0)}/${a.reduce((s2, c) => s2 + c.tentativas, 0)}** contra **B ${b.reduce((s2, c) => s2 + c.acertos, 0)}/${b.reduce((s2, c) => s2 + c.tentativas, 0)}**`,
      `- Prompt: **${media(a, 'promptTokens')}** contra **${media(b, 'promptTokens')}** tokens`,
      `- Turno: **${(media(a, 'ms') / 1000).toFixed(1)}s** contra **${(media(b, 'ms') / 1000).toFixed(1)}s**`,
      '',
      'Token e latência são fáceis de ganhar piorando o comportamento. O veredito é a primeira linha.',
      '',
    )
  }

  writeFileSync(join(REPORT_DIR, 'prompt-benchmark.md'), lines.join('\n'), 'utf8')
}
