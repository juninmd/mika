import type { Page } from '@playwright/test'
import { expect, test } from './fixtures/homolog-app'
import { createRecorder } from './fixtures/recorder'

// Proves what the fake Ollama can't: the real local model choosing the tool with production system.md
// in the prompt. A small model misroutes sometimes, so the hard gate is only "turn completed and spoke".

const recorder = createRecorder('Homologação — roteamento de ferramenta pelo modelo local', 'tool-routing')
const { record } = recorder

type BridgeWindow = {
  electronAPI: {
    runtime: {
      onToolCall: (cb: (payload: { name: string }) => void) => void
      onToolApprovalRequest: (cb: (req: { toolCallId: string; toolName: string }) => void) => void
      sendToolApprovalResponse: (id: string, approved: boolean, scope?: string) => void
      sendChatMessage: (
      text: string,
      attachments?: unknown,
      source?: string,
    ) => Promise<{ content?: string; role?: string }>
      clearChatHistory: () => Promise<unknown>
    }
  }
}

const CASOS = [
  { fala: 'Que horas são e quanto de RAM esse PC tem?', espera: 'get_system_info' },
  { fala: 'Me lembra daqui a 40 minutos de tomar água.', espera: 'reminder' },
  { fala: 'Qual foi meu tempo de jogo hoje?', espera: 'game_session' },
  { fala: 'Anota que meu jogo favorito é Hollow Knight.', espera: 'user_profile' },
  { fala: 'Bom dia! Dormiu bem?', espera: null },
] as const

async function armarEspiao(window: Page): Promise<void> {
  await window.evaluate(() => {
    const runtime = (window as unknown as BridgeWindow).electronAPI.runtime
    const chamadas: string[] = []
    ;(window as unknown as { __toolCalls: string[] }).__toolCalls = chamadas
    runtime.onToolCall((payload) => chamadas.push(payload.name))
    // Without a response to the approval request, the tool call hangs and the turn ends without it.
    runtime.onToolApprovalRequest((req) => runtime.sendToolApprovalResponse(req.toolCallId, true, 'once'))
  })
}

async function turno(window: Page, fala: string): Promise<{ resposta: string; falhou: boolean; ferramentas: string[] }> {
  return await window.evaluate(async (texto) => {
    const runtime = (window as unknown as BridgeWindow).electronAPI.runtime
    await runtime.clearChatHistory()
    ;(window as unknown as { __toolCalls: string[] }).__toolCalls.length = 0
    const reply = await runtime.sendChatMessage(texto, undefined, 'text')
    return {
      resposta: reply.content ?? '',
      // role: 'system' means the turn died — content is the app's error message, not her speech.
      falhou: reply.role === 'system',
      ferramentas: (window as unknown as { __toolCalls: string[] }).__toolCalls.slice(),
    }
  }, fala)
}

test('o modelo local escolhe a ferramenta sozinho', async ({ window }) => {
  test.setTimeout(900_000)
  await armarEspiao(window)

  let respondeuAlgo = 0
  const erros: string[] = []
  for (const caso of CASOS) {
    const { resposta, falhou, ferramentas } = await turno(window, caso.fala)
    if (falhou) erros.push(resposta)
    // Silence WITH a tool call is a success per the turn's own contract; only silence with nothing is a failure.
    if (!falhou && (resposta.trim().length > 0 || ferramentas.length > 0)) respondeuAlgo++

    const chamou = caso.espera ? ferramentas.some((nome) => nome.includes(caso.espera)) : ferramentas.length === 0
    record(
      'chat',
      `"${caso.fala}" → ${caso.espera ?? 'nenhuma ferramenta'}`,
      chamou ? 'ok' : 'falha',
      `chamou [${ferramentas.join(', ') || '—'}] · resposta: ${resposta.slice(0, 120)}`,
    )
  }

  // Hard gate is only turn completion and reply; routing accuracy goes to the report, not expect.
  expect(erros, `o turno morreu e o app devolveu erro: ${erros[0] ?? ''}`).toEqual([])
  expect(respondeuAlgo, 'turno sem resposta E sem ferramenta — o pipeline de chat não fechou').toBe(CASOS.length)
})

test.afterAll(() => recorder.flush())
