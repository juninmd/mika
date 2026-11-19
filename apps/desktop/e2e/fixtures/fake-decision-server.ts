/**
 * Loopback stand-in for `llama-server /v1/systemone` running the d1-2B decision model. The answer
 * shape follows llama.cpp's `tests/unit/test_systemone.py`; `verdict` picks a calm or an urgent
 * frame so the spec drives the watcher gate deterministically. Real-model proof is the ignored
 * `vision::decision::tests::decision_smoke_report`.
 */
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'

export type DecisionVerdict = 'calm' | 'urgent'

export interface FakeDecision {
  baseUrl: string
  verdict: DecisionVerdict
  /** Frames received (one `data:` image each). */
  requests: number
  close: () => Promise<void>
}

const noul = (p: number) => ({ type: 'noul', noul: p })

function answers(verdict: DecisionVerdict) {
  const urgent = verdict === 'urgent'
  return {
    game: noul(0.98),
    moment: { type: 'choice', choice: 'none', confidence: 0.9 },
    danger: noul(urgent ? 0.9 : 0.05),
    notable: noul(urgent ? 0.8 : 0.05),
  }
}

export async function startFakeDecision(): Promise<FakeDecision> {
  const state: FakeDecision = { baseUrl: '', verdict: 'calm', requests: 0, close: async () => {} }
  const server: Server = createServer((req, res) => {
    let raw = ''
    req.on('data', (chunk) => {
      raw += chunk
    })
    req.on('end', () => {
      const body = JSON.parse(raw || '{}') as { images?: string[] }
      if (req.url !== '/v1/systemone' || !body.images?.[0]?.startsWith('data:image/')) {
        res.writeHead(400).end()
        return
      }
      state.requests += 1
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ answers: answers(state.verdict), usage: { input_tokens: 1, output_tokens: 0 } }))
    })
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  state.baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
  state.close = () => new Promise((resolve) => server.close(() => resolve()))
  return state
}
