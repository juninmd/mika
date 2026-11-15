import { expect } from '@playwright/test'
import type { FakeOllama } from './fake-ollama-server'

type Body = { messages?: { role: string; content: string }[] }

// O que o modelo recebe como instrução do app num turno de chat: o system (fixo, cacheável) MAIS o
// contexto do turno colado à última mensagem do usuário (`TurnPrompt::user_message` em Rust). Com
// contexto vazio a mensagem sai idêntica ao texto do usuário. O pós-turno reenvia o texto puro com
// outro prompt — por isso o casamento com o marcador de contexto vem antes do exato.
const MARK = '\n\n[TURN CONTEXT'

export function outgoingSystem(llm: FakeOllama, userText: string, from = 0): string {
  const bodies = llm.requests.slice(from).map((request) => request.body as Body)
  const user = (body: Body) => body.messages?.findLast((message) => message.role === 'user')?.content ?? ''
  const body =
    bodies.find((b) => user(b).startsWith(userText + MARK)) ?? bodies.find((b) => user(b) === userText)
  expect(body, 'turno de chat real enviado ao provedor').toBeDefined()
  const system = body?.messages?.filter((message) => message.role === 'system').map((message) => message.content) ?? []
  const context = body ? user(body).slice(userText.length) : ''
  return [...system, context.trim()].filter(Boolean).join('\n')
}
