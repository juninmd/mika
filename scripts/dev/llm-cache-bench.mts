// Bench do LLM local (Ollama): mede, turno a turno, quanto do prompt o Ollama reprocessa e o tempo até
// o primeiro token, nos dois formatos de prompt que a Mika já usou.
//
// - `legado`: hora/tela/perfil dentro do system prompt — muda a cada turno, então o histórico inteiro
//   é reprocessado a cada fala.
// - `split`: o system fica fixo e o contexto do turno vai colado à última mensagem do usuário
//   (`brain::persona::system_prompt::TurnPrompt`) — o prefixo system + histórico é reaproveitado.
//
// Roda no PC (a nuvem não instala o Ollama): `pnpm bench:llm`. Variáveis opcionais: OLLAMA_URL
// (padrão http://127.0.0.1:11434), MODEL (padrão qwen3.5:4b), TURNS (padrão 10), NUM_CTX (32768,
// o padrão do app), OUT (arquivo JSON do relatório).
import { readFileSync, writeFileSync } from 'node:fs'

const OLLAMA = process.env.OLLAMA_URL ?? 'http://127.0.0.1:11434'
const MODEL = process.env.MODEL ?? 'qwen3.5:4b'
const TURNS = Number(process.env.TURNS ?? 10)
const NUM_CTX = Number(process.env.NUM_CTX ?? 32768)
const OUT = process.env.OUT ?? 'llm-cache-bench.json'

const prompt = (name: string) => readFileSync(new URL(`../../apps/desktop/prompts/brain/${name}.md`, import.meta.url), 'utf8')
const STABLE = prompt('system').replaceAll(/\{\{\w+\}\}/g, 'Mika')
const TAIL = prompt('system-tail').replaceAll('{{name}}', 'Mika').replaceAll('{{language}}', 'Brazilian Portuguese')

const MESSAGES = [
  'oi Mika, tudo bem?',
  'tô jogando Elden Ring e morri pro Margit de novo',
  'qual arma você acha melhor pra iniciante?',
  'e os talismãs, vale a pena farmar agora?',
  'mudando de assunto, me ajuda com um bug em Rust?',
  'o borrow checker reclama que eu uso o valor depois do move',
  'resolvi com clone, mas isso é feio né?',
  'beleza, voltei pro jogo. cheguei em Liurnia',
  'tem algum lugar bom pra subir de nível lá?',
  'valeu, vou parar por hoje',
  'boa noite Mika',
  'amanhã continuo',
]

type Mode = 'legado' | 'split'
type Msg = { role: 'system' | 'user' | 'assistant'; content: string }
type Turn = { turn: number; promptTokens: number; promptEvalMs: number; firstTokenMs: number; totalMs: number }

const volatile = (turn: number) =>
  `## TIME AWARENESS\n- **Current Time:** ${new Date().toISOString()}\n\n## CURRENT ENVIRONMENT CONTEXT\nForeground window: game, turn ${turn}.\n\n${TAIL}`

function request(mode: Mode, history: Msg[], text: string, turn: number): Msg[] {
  const context = volatile(turn)
  if (mode === 'legado') return [{ role: 'system', content: `${STABLE}\n\n${context}` }, ...history, { role: 'user', content: text }]
  const user = `${text}\n\n[TURN CONTEXT — written by the app, not by the user; never quote or mention it]\n${context}`
  return [{ role: 'system', content: STABLE }, ...history, { role: 'user', content: user }]
}

async function ollama(path: string, body: unknown): Promise<Record<string, unknown>> {
  const res = await fetch(`${OLLAMA}${path}`, { method: 'POST', body: JSON.stringify(body) })
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status} ${await res.text()}`)
  return (await res.json()) as Record<string, unknown>
}

const ms = (ns: unknown) => Math.round(Number(ns ?? 0) / 1e6)

async function runTurn(messages: Msg[]): Promise<{ reply: string; stats: Omit<Turn, 'turn'> }> {
  const started = performance.now()
  const res = await fetch(`${OLLAMA}/api/chat`, {
    method: 'POST',
    body: JSON.stringify({ model: MODEL, messages, stream: true, think: false, keep_alive: '30m', options: { num_ctx: NUM_CTX, temperature: 0, seed: 7 } }),
  })
  if (!res.ok || !res.body) throw new Error(`/api/chat: HTTP ${res.status}`)
  let reply = ''
  let firstTokenMs = 0
  let last: Record<string, unknown> = {}
  const decoder = new TextDecoder()
  let buffer = ''
  for await (const chunk of res.body) {
    buffer += decoder.decode(chunk, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    for (const line of lines.filter(Boolean)) {
      last = JSON.parse(line)
      const piece = (last.message as { content?: string } | undefined)?.content ?? ''
      if (piece && !firstTokenMs) firstTokenMs = Math.round(performance.now() - started)
      reply += piece
    }
  }
  const stats = { promptTokens: Number(last.prompt_eval_count ?? 0), promptEvalMs: ms(last.prompt_eval_duration), firstTokenMs, totalMs: Math.round(performance.now() - started) }
  return { reply, stats }
}

async function runMode(mode: Mode): Promise<Turn[]> {
  await ollama('/api/generate', { model: MODEL, keep_alive: 0 }) // descarrega: nenhum cache herdado do modo anterior
  await ollama('/api/generate', { model: MODEL, keep_alive: '30m', options: { num_ctx: NUM_CTX } })
  const history: Msg[] = []
  const turns: Turn[] = []
  for (const [index, text] of MESSAGES.slice(0, TURNS).entries()) {
    const { reply, stats } = await runTurn(request(mode, history, text, index + 1))
    history.push({ role: 'user', content: text }, { role: 'assistant', content: reply })
    turns.push({ turn: index + 1, ...stats })
    console.log(`${mode} turno ${index + 1}: ${stats.promptTokens} tokens reprocessados, 1º token em ${stats.firstTokenMs} ms`)
  }
  return turns
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0
const summary = (turns: Turn[]) => {
  const later = turns.slice(1) // o 1º turno não tem histórico para reaproveitar nos dois modos
  return { medianFirstTokenMs: median(later.map((t) => t.firstTokenMs)), medianPromptTokens: median(later.map((t) => t.promptTokens)) }
}

const legado = await runMode('legado')
const split = await runMode('split')
const report = { model: MODEL, numCtx: NUM_CTX, turns: TURNS, date: new Date().toISOString(), legado: { summary: summary(legado), turns: legado }, split: { summary: summary(split), turns: split } }
writeFileSync(OUT, `${JSON.stringify(report, null, 2)}\n`)
console.log(`\nMediana do 1º token (turnos 2+): legado ${report.legado.summary.medianFirstTokenMs} ms, split ${report.split.summary.medianFirstTokenMs} ms`)
console.log(`Relatório: ${OUT}`)
