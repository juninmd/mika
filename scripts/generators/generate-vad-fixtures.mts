/**
 * pnpm vad:fixtures — áudios para provar que a detecção de voz separa fala de barulho de teclado.
 *
 * Gera em `apps/desktop/e2e/fixtures/voice/noise/` (PCM16 mono 16 kHz, o formato do backend):
 * digitação sintética (mecânica, membrana, rajadas com espaço, perto do mic), cliques de mouse,
 * ventoinha, e misturas com as falas REAIS/TTS já versionadas em `fixtures/voice/`. O manifesto
 * `vad-cases.json` diz, por arquivo, se há fala e em que trechos — o gabarito dos testes.
 *
 * Determinístico: PRNG com semente fixa, então rodar de novo produz os mesmos bytes. A digitação é
 * sintética de propósito: um clique de tecla é um transiente curto (ataque < 1 ms, decaimento de
 * 10–30 ms), de banda larga e sem estrutura harmônica — exatamente o que o detector precisa rejeitar.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '..', '..')
const VOICE_DIR = join(ROOT, 'apps', 'desktop', 'e2e', 'fixtures', 'voice')
const OUT_DIR = join(VOICE_DIR, 'noise')
const RATE = 16_000

function rng(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) >>> 0
    let t = s
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

function readWav(path: string): Float32Array {
  const buf = readFileSync(path)
  let offset = 12
  while (offset < buf.length) {
    const id = buf.toString('ascii', offset, offset + 4)
    const size = buf.readUInt32LE(offset + 4)
    if (id === 'fmt ') {
      const channels = buf.readUInt16LE(offset + 10)
      const rate = buf.readUInt32LE(offset + 12)
      const bits = buf.readUInt16LE(offset + 22)
      if (channels !== 1 || rate !== RATE || bits !== 16) throw new Error(`${path}: precisa PCM16 mono 16 kHz`)
    }
    if (id === 'data') {
      const out = new Float32Array(size / 2)
      for (let i = 0; i < out.length; i++) out[i] = buf.readInt16LE(offset + 8 + i * 2) / 32768
      return out
    }
    offset += 8 + size
  }
  throw new Error(`${path}: sem chunk data`)
}

function writeWav(path: string, samples: Float32Array): void {
  const data = Buffer.alloc(samples.length * 2)
  for (let i = 0; i < samples.length; i++) {
    const v = Math.max(-1, Math.min(1, samples[i] ?? 0))
    data.writeInt16LE(Math.round(v * 32767), i * 2)
  }
  const header = Buffer.alloc(44)
  header.write('RIFF', 0)
  header.writeUInt32LE(36 + data.length, 4)
  header.write('WAVEfmt ', 8)
  header.writeUInt32LE(16, 16)
  header.writeUInt16LE(1, 20)
  header.writeUInt16LE(1, 22)
  header.writeUInt32LE(RATE, 24)
  header.writeUInt32LE(RATE * 2, 28)
  header.writeUInt16LE(2, 32)
  header.writeUInt16LE(16, 34)
  header.write('data', 36)
  header.writeUInt32LE(data.length, 40)
  writeFileSync(path, Buffer.concat([header, data]))
}

interface Click {
  gain: number
  decayMs: number
  /** Ressonância do corpo da tecla (Hz) — mecânica ~3–5 kHz, membrana ~1–2 kHz. */
  bodyHz: number
  /** "Fundo de curso": batida grave logo depois do clique. */
  thudHz: number
}

/** Um clique: ruído com ataque instantâneo, decaimento exponencial, corpo ressonante e batida grave. */
function addClick(out: Float32Array, at: number, c: Click, rand: () => number): void {
  const start = Math.floor(at * RATE)
  const len = Math.floor((c.decayMs / 1000) * RATE * 4)
  const tau = (c.decayMs / 1000) * RATE
  let lp = 0
  for (let i = 0; i < len && start + i < out.length; i++) {
    const env = Math.exp(-i / tau)
    const noise = rand() * 2 - 1
    lp += 0.35 * (noise - lp)
    const body = Math.sin((2 * Math.PI * c.bodyHz * i) / RATE) * 0.6
    const thud =
      i > tau * 0.6 ? Math.sin((2 * Math.PI * c.thudHz * i) / RATE) * Math.exp(-(i - tau * 0.6) / (tau * 1.5)) * 0.5 : 0
    out[start + i] += c.gain * (env * (0.7 * (noise - lp) + body * noise) + thud)
  }
}

const MECHANICAL: Click = { gain: 0.45, decayMs: 12, bodyHz: 4200, thudHz: 180 }
const MEMBRANE: Click = { gain: 0.22, decayMs: 22, bodyHz: 1400, thudHz: 120 }
const SPACEBAR: Click = { gain: 0.5, decayMs: 30, bodyHz: 900, thudHz: 90 }
const MOUSE: Click = { gain: 0.3, decayMs: 6, bodyHz: 5200, thudHz: 300 }

/** Digitação humana: rajadas de "palavras" (3–8 teclas, 70–160 ms entre elas) separadas por espaço. */
function typing(seconds: number, click: Click, seed: number, keysPerSec = 7): Float32Array {
  const out = new Float32Array(Math.floor(seconds * RATE))
  const rand = rng(seed)
  let t = 0.1
  while (t < seconds - 0.2) {
    const word = 3 + Math.floor(rand() * 6)
    for (let k = 0; k < word && t < seconds - 0.2; k++) {
      addClick(out, t, { ...click, gain: click.gain * (0.7 + rand() * 0.5) }, rand)
      t += (1 / keysPerSec) * (0.6 + rand() * 0.8)
    }
    addClick(out, t, SPACEBAR, rand)
    t += 0.12 + rand() * (rand() < 0.15 ? 0.9 : 0.25)
  }
  return out
}

function mouseClicks(seconds: number, seed: number): Float32Array {
  const out = new Float32Array(Math.floor(seconds * RATE))
  const rand = rng(seed)
  for (let t = 0.3; t < seconds - 0.2; t += 0.4 + rand() * 1.2) {
    addClick(out, t, MOUSE, rand)
    if (rand() < 0.3) addClick(out, t + 0.09, MOUSE, rand)
  }
  return out
}

/** Ventoinha: ruído marrom + zumbido de 120 Hz e harmônicos — ruído estacionário, não fala. */
function fan(seconds: number, gain: number, seed: number): Float32Array {
  const out = new Float32Array(Math.floor(seconds * RATE))
  const rand = rng(seed)
  let brown = 0
  for (let i = 0; i < out.length; i++) {
    brown = (brown + (rand() * 2 - 1) * 0.02) * 0.998
    const hum = Math.sin((2 * Math.PI * 120 * i) / RATE) * 0.3 + Math.sin((2 * Math.PI * 240 * i) / RATE) * 0.1
    out[i] = gain * (brown * 3 + hum * 0.2)
  }
  return out
}

function silence(seconds: number): Float32Array {
  return new Float32Array(Math.floor(seconds * RATE))
}

function scaled(signal: Float32Array, gain: number): Float32Array {
  return signal.map((v) => v * gain)
}

/** Soma `add` em `base` a partir de `atSec`; devolve uma cópia do tamanho do maior. */
function mix(base: Float32Array, add: Float32Array, atSec = 0): Float32Array {
  const at = Math.floor(atSec * RATE)
  const out = new Float32Array(Math.max(base.length, at + add.length))
  out.set(base)
  for (let i = 0; i < add.length; i++) out[at + i] += add[i] ?? 0
  return out
}

/** Onde há fala num clipe limpo: primeiro e último quadro de 20 ms acima de 10% do pico RMS. */
function speechBounds(clean: Float32Array): [number, number] {
  const frame = RATE / 50
  const rms: number[] = []
  for (let i = 0; i + frame <= clean.length; i += frame) {
    let sum = 0
    for (let j = 0; j < frame; j++) sum += (clean[i + j] ?? 0) ** 2
    rms.push(Math.sqrt(sum / frame))
  }
  const limit = Math.max(...rms) * 0.1
  const first = rms.findIndex((v) => v > limit)
  const last = rms.length - 1 - [...rms].reverse().findIndex((v) => v > limit)
  return [first / 50, (last + 1) / 50]
}

interface Case {
  file: string
  description: string
  expectSpeech: boolean
  speechSegments: [number, number][]
  noise: string[]
}

const round = (n: number) => Math.round(n * 100) / 100

function main(): void {
  mkdirSync(OUT_DIR, { recursive: true })
  const speech = readWav(join(VOICE_DIR, 'fala-usuario-pt-br.wav'))
  const command = readWav(join(VOICE_DIR, 'commands', 'abrir-site.wav'))
  const cases: Case[] = []
  const emit = (
    file: string,
    description: string,
    samples: Float32Array,
    noise: string[],
    segments: [number, number][] = [],
  ) => {
    writeWav(join(OUT_DIR, file), samples)
    cases.push({
      file,
      description,
      expectSpeech: segments.length > 0,
      speechSegments: segments.map(([a, b]) => [round(a), round(b)]),
      noise,
    })
  }
  const placed = (clean: Float32Array, at: number): [number, number] => {
    const [a, b] = speechBounds(clean)
    return [a + at, b + at]
  }

  emit('keyboard-mechanical.wav', 'Teclado mecânico, ~8 teclas/s, 10 s', typing(10, MECHANICAL, 1, 8), ['keyboard'])
  emit('keyboard-membrane.wav', 'Teclado de membrana, ~5 teclas/s, 10 s', typing(10, MEMBRANE, 2, 5), ['keyboard'])
  emit(
    'keyboard-close-mic.wav',
    'Teclado mecânico colado no microfone (muito alto), 8 s',
    scaled(typing(8, MECHANICAL, 3, 9), 1.8),
    ['keyboard'],
  )
  emit('mouse-clicks.wav', 'Cliques e duplo-cliques de mouse, 8 s', mouseClicks(8, 4), ['mouse'])
  emit('fan-room-tone.wav', 'Ventoinha / ruído de sala, 8 s', fan(8, 0.05, 5), ['fan'])
  emit('keyboard-and-fan.wav', 'Digitação sobre ventoinha, 10 s', mix(typing(10, MECHANICAL, 6, 7), fan(10, 0.04, 7)), [
    'keyboard',
    'fan',
  ])

  const speechAt = 2
  emit(
    'speech-with-typing.wav',
    'Fala real do usuário com digitação por baixo o tempo todo',
    mix(mix(silence(speech.length / RATE + 4), speech, speechAt), typing(speech.length / RATE + 4, MECHANICAL, 8, 7)),
    ['keyboard'],
    [placed(speech, speechAt)],
  )
  const typedFor = 4
  emit(
    'typing-then-command.wav',
    '4 s só digitando, depois "abrir site" (TTS) — a fala deve começar em ~4 s, não antes',
    mix(
      typing(typedFor + command.length / RATE + 1, MEMBRANE, 9, 6).map((v, i) => (i < typedFor * RATE ? v : v * 0.3)),
      command,
      typedFor,
    ),
    ['keyboard'],
    [placed(command, typedFor)],
  )
  emit(
    'quiet-speech-with-fan.wav',
    'Fala real a -12 dB sobre ventoinha — tem de continuar sendo fala',
    mix(fan(speech.length / RATE + 3, 0.04, 10), scaled(speech, 0.25), 1.5),
    ['fan'],
    [placed(speech, 1.5)],
  )
  emit(
    'command-with-mouse.wav',
    'Comando (TTS) com cliques de mouse antes e durante',
    mix(mouseClicks(command.length / RATE + 4, 11), command, 2),
    ['mouse'],
    [placed(command, 2)],
  )
  emit('silence.wav', 'Silêncio digital, 5 s', silence(5), [])

  writeFileSync(join(OUT_DIR, 'vad-cases.json'), `${JSON.stringify({ sampleRate: RATE, cases }, null, 2)}\n`)
  console.log(`${cases.length} fixtures em ${OUT_DIR}`)
}

main()
