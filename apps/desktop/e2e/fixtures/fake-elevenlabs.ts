import type { IncomingMessage, ServerResponse } from 'node:http'

/** Chave salva pelo spec; o dublê só responde a ela, provando que o backend mandou a REAL, não a mascarada. */
export const ELEVENLABS_TEST_KEY = 'xi-chave-de-teste-nunca-real'

/** 1 s de silêncio em WAV PCM 8-bit 8 kHz: prévia tocável sem rede (a CSP aceita `data:`). */
function silentWavDataUrl(): string {
  const samples = 8000
  const buf = Buffer.alloc(44 + samples, 0x80)
  buf.write('RIFF', 0)
  buf.writeUInt32LE(36 + samples, 4)
  buf.write('WAVEfmt ', 8)
  buf.writeUInt32LE(16, 16)
  buf.writeUInt16LE(1, 20)
  buf.writeUInt16LE(1, 22)
  buf.writeUInt32LE(8000, 24)
  buf.writeUInt32LE(8000, 28)
  buf.writeUInt16LE(1, 32)
  buf.writeUInt16LE(8, 34)
  buf.write('data', 36)
  buf.writeUInt32LE(samples, 40)
  return `data:audio/wav;base64,${buf.toString('base64')}`
}

export const ELEVENLABS_VOICES = [
  { voice_id: 'voz-aurora', name: 'Aurora', category: 'premade', labels: { gender: 'female', accent: 'brazilian' } },
  { voice_id: 'voz-bento', name: 'Bento', category: 'premade', labels: { gender: 'male', accent: 'brazilian' } },
].map((v) => ({ ...v, preview_url: silentWavDataUrl() }))

/** `GET /v1/user/subscription`: cota do plano em caracteres (formato da API oficial). */
export const ELEVENLABS_SUBSCRIPTION = {
  tier: 'starter',
  character_count: 2500,
  character_limit: 30000,
  next_character_count_reset_unix: 1_798_761_600,
  status: 'active',
}

const ELEVENLABS_ROUTES: Record<string, () => unknown> = {
  '/v1/voices': () => ({ voices: ELEVENLABS_VOICES }),
  '/v1/user/subscription': () => ELEVENLABS_SUBSCRIPTION,
}

/** Rotas da ElevenLabs (`/v1/voices`, `/v1/user/subscription`). Devolve `true` quando atendeu a rota. */
export function handleElevenLabs(path: string, req: IncomingMessage, res: ServerResponse): boolean {
  const route = ELEVENLABS_ROUTES[path]
  if (!route) return false
  if (req.headers['xi-api-key'] !== ELEVENLABS_TEST_KEY) {
    res.writeHead(401, { 'Content-Type': 'application/json' })
    res.end(JSON.stringify({ detail: { status: 'invalid_api_key' } }))
    return true
  }
  res.writeHead(200, { 'Content-Type': 'application/json' })
  res.end(JSON.stringify(route()))
  return true
}
