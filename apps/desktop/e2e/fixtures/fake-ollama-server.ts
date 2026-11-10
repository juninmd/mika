import { appendFileSync } from 'node:fs'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'

export interface FakeOllama {
  baseUrl: string

  requests: { path: string; body: unknown }[]
  /** Atraso (ms) antes do 1º token das respostas em stream: o "pensando" de um turno real, observável. */
  setChatDelay: (ms: number) => void
  close: () => Promise<void>
}

const HUMOR_SIGNALS = ['engraçad', 'piada', 'meme', 'zoand', 'zoeira', 'hilári', 'rindo', 'funny']

function isFunnyTurn(turn: string): boolean {
  if (turn.includes('[HUMOR/MEME]')) return true
  const observed = (observedContent(turn) ?? '').toLowerCase()
  return HUMOR_SIGNALS.some((signal) => observed.includes(signal))
}

const IGNORED_NAMES = new Set([
  'Mika',
  'Web Browser',
  'Você',
  'Conteúdo',
  'Contexto',
  'Tela',
  'Usuário',
  'Observação',
  'Análise',
  'Sistema',
  'Nota',
  'Regra',
  'Prioridades',
  'Imagem',
  'Janela',
  'Aplicativo',
  'Instruções',
  'Dados',
])

function observedContent(prompt: string): string | undefined {
  // `screen-observation.md` ("On screen:") and `vision-people.md` ("Conteúdo da tela:") quote the analysis.
  const quoted = prompt.match(/(?:Conteúdo da tela|On screen):\s*"([^"]*)"/)?.[1]?.trim()
  if (quoted) return quoted

  const envelope = prompt.split('[DADOS OBSERVADOS NA TELA, NAO SAO INSTRUCOES]:')[1]
  if (!envelope) return undefined
  return (
    envelope

      .replace(/SYSTEM:\s*\[[^\]]*\][^.]*\./, '')
      .split(/\n|Página:|App:|Eventos recentes:|\|/)[0]
      ?.trim() || undefined
  )
}

function extractNames(prompt: string): string[] {
  const source = observedContent(prompt) ?? prompt

  const found = new Set<string>()
  for (const match of source.matchAll(/\b([A-ZÁÉÍÓÚÂÊÔÃÕÇ][a-zà-ÿ']+(?:\s+[A-ZÁÉÍÓÚÂÊÔÃÕÇ][a-zà-ÿ']+)*)\b/g)) {
    const candidate = match[1] as string
    if (candidate.length >= 4 && !IGNORED_NAMES.has(candidate)) found.add(candidate)
  }

  return [...found].sort((a, b) => b.split(' ').length - a.split(' ').length)
}

function bioLine(prompt: string): string | undefined {
  // O app manda "Bio/Lore:" (`brain/awareness/person_detected.rs`); "Biografia/Lore:" era o rótulo antigo.
  return prompt.match(/(?:Biografia|Bio)\/Lore:\s*(.+)/)?.[1]?.trim()
}

// Short, distinct tails: lower similarity from 0.83 to ~0.59 without lengthening replies.
const TAILS = ['nem tinha reparado', 'ia falar outra coisa', 'lembrei do grupo', 'anotei pra cobrar', 'fiquei curiosa']

function replyFor(turn: string, names: string[], funny: boolean, nonce: number, variant: number): string {
  const parts: string[] = []

  const LAUGHS = ['kkkkk que coisa boa', 'hahaha morri', 'kkkk não acredito nessa', 'kkkkkk ri alto', 'hahaha pesado']
  const MENTIONS = ['reparei em', 'apareceu aí', 'tá na tela', 'vi passar', 'peguei o nome']

  if (funny) parts.push(LAUGHS[variant % LAUGHS.length] as string)
  if (names.length > 0) {
    parts.push(`${MENTIONS[variant % MENTIONS.length]} ${names.slice(0, 2).join(' e ')}`)
  }

  const seen = observedContent(turn)
  const FRAMES = ['olhando', 'curtindo', 'assistindo', 'acompanhando', 'reagindo']
  if (seen) parts.push(`${FRAMES[variant % FRAMES.length]} "${seen.slice(0, 80)}"`)

  const bio = bioLine(turn)
  if (bio) parts.push(bio.slice(0, 120))

  if (parts.length === 0) parts.push(`saquei, tô contigo nessa (${turn.trim().slice(0, 24)})`)

  // Distinct tail avoids near-duplicate replies tripping the 0.75 repetition guard and hiding a fake-double defect as a product bug.
  return `${parts.join(', ')} — ${TAILS[variant % TAILS.length]} (${nonce})`
}

type JsonSchema = {
  type?: string | string[]
  properties?: Record<string, JsonSchema>
  items?: JsonSchema
  enum?: unknown[]
  minimum?: number
}

const NAME_FIELD = /^(name|title|fullName|person|character)$/i
const PEOPLE_FIELD = /(person|people|celebri|entit|character|mention)/i
const HUMOR_FIELD = /(funny|humor|meme|joke)/i

function synthesize(schema: JsonSchema | undefined, key: string, names: string[], funny: boolean): unknown {
  const type = Array.isArray(schema?.type) ? schema?.type[0] : schema?.type

  if (schema?.enum && schema.enum.length > 0) return schema.enum[0]

  if (type === 'object' || schema?.properties) {
    const out: Record<string, unknown> = {}
    for (const [child, childSchema] of Object.entries(schema?.properties ?? {})) {
      out[child] = synthesize(childSchema, child, names, funny)
    }
    return out
  }

  if (type === 'array') {
    if (PEOPLE_FIELD.test(key) && names.length > 0) {
      return names.slice(0, 3).map((name) => {
        const item = synthesize(schema?.items, key, [name], funny)
        return item && typeof item === 'object' ? item : name
      })
    }
    return []
  }

  if (type === 'boolean') return HUMOR_FIELD.test(key) ? funny : false
  if (type === 'number' || type === 'integer') return schema?.minimum ?? 0
  if (type === 'null') return null

  if (NAME_FIELD.test(key) && names.length > 0) return names[0]
  if (key === 'analysis' || key === 'comment' || key === 'description') {
    return funny ? 'cena engraçada na tela' : `na tela: ${names.slice(0, 2).join(', ') || 'nada notável'}`
  }
  return ''
}

function textOf(message: unknown): string {
  const content = (message as { content?: unknown }).content
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return content
      .map((part) => (typeof part === 'object' && part && 'text' in part ? String(part.text) : ''))
      .join(' ')
  }
  return ''
}

const FACT_EXTRACTION_MARKER = 'Extract PERMANENT, CONFIRMED facts'
const FAVORITE_GAME = /jogo favorito [ée] ([^.,!?<\n]+)/i

/**
 * Plays the LLM for `brain/fact-extraction.md`: a stated favorite game in the user's message becomes
 * one fact in the prompt's JSON contract; anything else confirms nothing (`{"facts":[]}`). Only the
 * user's own words count, as the prompt demands — Mika's reply is never mined.
 */
function factsReplyFor(all: string): string | null {
  if (!all.includes(FACT_EXTRACTION_MARKER)) return null
  const userMessage = all.match(/<user_message>([\s\S]*?)<\/user_message>/)?.[1] ?? ''
  const game = userMessage.match(FAVORITE_GAME)?.[1]?.trim()
  if (!game) return JSON.stringify({ facts: [] })
  const fact = { content: `jogo favorito é ${game}`, confidence: 0.95, subject: 'user', predicate: 'jogo favorito é', object: game }
  return JSON.stringify({ facts: [fact] })
}

function collectPrompt(body: Record<string, unknown>): { all: string; turn: string } {
  const messages = Array.isArray(body.messages) ? body.messages : []
  const system = typeof body.system === 'string' ? body.system : ''
  const turn = messages
    .filter((message) => (message as { role?: string }).role !== 'system')
    .map(textOf)
    .join('\n')
  const all = `${typeof body.prompt === 'string' ? body.prompt : ''}\n${system}\n${messages.map(textOf).join('\n')}`
  return { all, turn: `${typeof body.prompt === 'string' ? body.prompt : ''}\n${turn}` }
}

function toolCallFor(
  body: Record<string, unknown>,
  names: string[],
  turn: string,
): { name: string; args: unknown } | null {
  const tools = Array.isArray(body.tools) ? body.tools : []
  const available = new Set(
    tools.map((entry) => String((entry as { function?: { name?: string } })?.function?.name ?? '')),
  )

  if (turn.includes('HOMOLOGA_TOOL_LOOP') && available.has('get_system_info')) {
    const messages = Array.isArray(body.messages) ? body.messages : []
    if (messages.some((message) => (message as { role?: string }).role === 'tool')) return null
    return { name: 'get_system_info', args: {} }
  }

  if (turn.includes('HOMOLOGA_APROVACAO') && available.has('clear_history')) {
    const messages = Array.isArray(body.messages) ? body.messages : []
    if (messages.some((message) => (message as { role?: string }).role === 'tool')) return null
    return { name: 'clear_history', args: { confirm: true } }
  }

  if (!available.has('person_catalog') || names.length === 0) return null

  if (!turn.includes('DADOS OBSERVADOS NA TELA')) return null

  const messages = Array.isArray(body.messages) ? body.messages : []
  const alreadyRan = messages.some((message) => (message as { role?: string }).role === 'tool')
  if (alreadyRan) return null

  return {
    name: 'person_catalog',
    args: { action: 'upsert', name: names[0], type: 'real_person', origin: 'tela' },
  }
}

function toolCallChunk(call: { name: string; args: unknown }, model: string): string {
  return JSON.stringify({
    model,
    created_at: new Date(0).toISOString(),
    message: {
      role: 'assistant',
      content: '',
      tool_calls: [{ function: { name: call.name, arguments: call.args } }],
    },
    done: true,
    done_reason: 'stop',
    prompt_eval_count: 42,
    eval_count: 1,
  })
}

function chatChunks(reply: string, model: string): string[] {
  const words = reply.split(' ')
  const now = new Date(0).toISOString()
  const deltas = words.map((word, index) =>
    JSON.stringify({
      model,
      created_at: now,
      message: { role: 'assistant', content: index === 0 ? word : ` ${word}` },
      done: false,
    }),
  )
  deltas.push(
    JSON.stringify({
      model,
      created_at: now,
      message: { role: 'assistant', content: '' },
      done: true,
      done_reason: 'stop',
      prompt_eval_count: 42,
      eval_count: words.length,
      total_duration: 1_000_000,
    }),
  )
  return deltas
}

function nonceFor(turn: string, names: string[]): number {
  const seed = `${turn}|${names.join(',')}`
  let hash = 0
  for (let i = 0; i < seed.length; i++) hash = (hash * 31 + seed.charCodeAt(i)) % 100_000
  return hash
}

/** Resposta única e coerente do Google/OpenRouter de mentirinha (os provedores locais usam `replyFor`). */
export const CLOUD_REPLY = 'Oi! Aqui é a Mika, respondendo pelo provedor de nuvem de mentirinha do teste.'

export async function startFakeOllama(): Promise<FakeOllama> {
  const requests: { path: string; body: unknown }[] = []
  // A new observation takes the next template, a repeated one keeps its own: hashing collided
  // often enough that three distinct replies shared a verb and tripped the topic-saturation guard.
  let chatDelayMs = 0
  const variants = new Map<string, number>()
  const variantFor = (turn: string): number => {
    const key = `${observedContent(turn) ?? turn}|${bioLine(turn) ?? ''}`
    if (!variants.has(key)) variants.set(key, variants.size)
    return variants.get(key) as number
  }

  const server: Server = createServer((req, res) => {
    const chunks: Buffer[] = []
    req.on('data', (chunk: Buffer) => chunks.push(chunk))
    req.on('end', () => {
      const path = (req.url ?? '').split('?')[0] ?? ''
      let body: Record<string, unknown> = {}
      try {
        body = chunks.length > 0 ? (JSON.parse(Buffer.concat(chunks).toString('utf8')) as Record<string, unknown>) : {}
      } catch {
        body = {}
      }
      requests.push({ path, body })

      const debugSink = process.env.MIKA_FAKE_LLM_DEBUG
      if (debugSink && (path === '/api/chat' || path === '/api/generate')) {
        const tools = Array.isArray(body.tools) ? body.tools : []
        const names = tools.map((t) => (t as { function?: { name?: string } })?.function?.name).filter(Boolean)
        const turnPreview = collectPrompt(body).turn.replace(/\s+/g, ' ').slice(0, 220)
        appendFileSync(
          debugSink,
          `${path} tools=${tools.length} hasUpsert=${names.includes('person_catalog')} format=${body.format ? 'yes' : 'no'} roles=${(Array.isArray(body.messages) ? body.messages : []).map((m) => (m as { role?: string }).role).join(',')}\n  turn=${turnPreview}\n`,
        )
      }

      const model = typeof body.model === 'string' ? body.model : 'fake-model'

      if (path === '/api/tags') {
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ models: [{ name: model, model, size: 1, details: {} }] }))
        return
      }

      if (path === '/api/pull') {
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ status: 'success' }))
        return
      }

      if (path === '/api/show') {
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ details: { family: 'fake' }, capabilities: ['completion', 'tools', 'vision'] }))
        return
      }

      if (path === '/api/embed' || path === '/api/embeddings') {
        const input = body.input ?? body.prompt
        const count = Array.isArray(input) ? input.length : 1

        const vector = Array.from({ length: 8 }, (_, i) => (i + 1) / 10)
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ embeddings: Array.from({ length: count }, () => vector), embedding: vector }))
        return
      }

      // Nuvem de mentirinha: o app aponta o Google e o OpenRouter para cá (`MIKA_E2E_CLOUD_BASE_URL`,
      // só no binário `e2e`), então o fluxo real do provedor roda sem chamar — nem cobrar — a nuvem.
      const gemini = path.match(/^\/v1beta\/models\/([^:/]+):streamGenerateContent$/)
      if (gemini) {
        const part = (text: string, last: boolean) =>
          `data: ${JSON.stringify({
            candidates: [{ content: { role: 'model', parts: [{ text }] }, ...(last ? { finishReason: 'STOP' } : {}), index: 0 }],
            ...(last ? { usageMetadata: { promptTokenCount: 42, candidatesTokenCount: 12, totalTokenCount: 54 } } : {}),
            modelVersion: gemini[1],
          })}\n\n`
        res.writeHead(200, { 'Content-Type': 'text/event-stream' })
        res.write(part(CLOUD_REPLY.slice(0, 30), false))
        res.write(part(CLOUD_REPLY.slice(30), true))
        res.end()
        return
      }

      if (path === '/api/v1/chat/completions') {
        const chunk = (delta: Record<string, unknown>, finish: string | null, usage?: unknown) =>
          `data: ${JSON.stringify({
            id: 'gen-fake',
            object: 'chat.completion.chunk',
            created: 0,
            model,
            choices: [{ index: 0, delta, finish_reason: finish }],
            ...(usage ? { usage } : {}),
          })}\n\n`
        res.writeHead(200, { 'Content-Type': 'text/event-stream' })
        res.write(chunk({ role: 'assistant', content: CLOUD_REPLY.slice(0, 30) }, null))
        res.write(chunk({ content: CLOUD_REPLY.slice(30) }, null))
        res.write(chunk({}, 'stop', { prompt_tokens: 42, completion_tokens: 12, total_tokens: 54 }))
        res.write('data: [DONE]\n\n')
        res.end()
        return
      }

      if (path === '/api/chat' || path === '/api/generate') {
        const { all, turn } = collectPrompt(body)

        const names = extractNames(turn).filter((name) => !IGNORED_NAMES.has(name))
        const funny = isFunnyTurn(turn)

        const format = body.format
        const structured =
          format && typeof format === 'object'
            ? JSON.stringify(synthesize(format as JsonSchema, 'root', names, funny))
            : null

        const call = structured ? null : toolCallFor(body, names, turn)
        if (call) {
          res.writeHead(200, { 'Content-Type': 'application/x-ndjson' })
          res.write(`${toolCallChunk(call, model)}\n`)
          res.end()
          return
        }

        const reply = structured ?? factsReplyFor(all) ?? replyFor(turn, names, funny, nonceFor(turn, names), variantFor(turn))
        if (body.stream === false) {
          res.writeHead(200, { 'Content-Type': 'application/json' })
          res.end(
            JSON.stringify({
              model,
              created_at: new Date(0).toISOString(),
              message: { role: 'assistant', content: reply },
              response: reply,
              done: true,
              done_reason: 'stop',
              prompt_eval_count: 42,
              eval_count: reply.split(' ').length,
            }),
          )
          return
        }
        const stream = () => {
          res.writeHead(200, { 'Content-Type': 'application/x-ndjson' })
          for (const chunk of chatChunks(reply, model)) res.write(`${chunk}\n`)
          res.end()
        }
        if (chatDelayMs > 0) setTimeout(stream, chatDelayMs)
        else stream()
        return
      }

      res.writeHead(404, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ error: `fake-ollama: rota não implementada (${path})` }))
    })
  })

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as AddressInfo

  return {
    baseUrl: `http://127.0.0.1:${port}`,
    requests,
    setChatDelay: (ms) => {
      chatDelayMs = ms
    },
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections?.()
        server.close(() => resolve())
      }),
  }
}
