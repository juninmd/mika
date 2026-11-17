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

/** `GET /v1/voices` da ElevenLabs. Devolve `true` quando atendeu a rota. */
export function handleElevenLabs(path: string, req: IncomingMessage, res: ServerResponse): boolean {
  if (path !== '/v1/voices') return false
  if (req.headers['xi-api-key'] !== ELEVENLABS_TEST_KEY) {
    res.writeHead(401, { 'Content-Type': 'application/json' })
    res.end(JSON.stringify({ detail: { status: 'invalid_api_key' } }))
    return true
  }
  res.writeHead(200, { 'Content-Type': 'application/json' })
  res.end(JSON.stringify({ voices: ELEVENLABS_VOICES }))
  return true
}
