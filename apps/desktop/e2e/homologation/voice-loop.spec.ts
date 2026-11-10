import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './fixtures/homolog-app'

// Homologates the full conversation chain (TTS → VAD → Whisper → LLM → TTS): Mika speaks, her own audio
// loops back through the real mic transcription path with no human needed. Negative case matters equally:
// silence and white noise must come back EMPTY.

const PHRASES = [
  'Bom dia, Mika, tudo bem com você hoje?',
  'Qual é a capital do Brasil?',
  'Estou jogando Counter-Strike agora.',
  'Me explica o que é um buraco negro.',
  'Prefiro sushi a pizza, e você?',
]

const REPORT_DIR = join(__dirname, 'report')

type TtsChunk = { audio: string; mimeType: string }

type LegResult = {
  frase: string
  ttsChunks: number
  audioMs: number
  transcricao: string
  similaridade: number
  rms: number
  peak: number
  resposta: string
  respostaMs: number
  ferramentas: string[]
  erro: string
}

function normalize(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9\s]/g, ' ')
    .trim()
    .replace(/\s+/g, ' ')
}

function similarity(spoken: string, heard: string): number {
  const said = normalize(spoken).split(' ').filter(Boolean)
  const got = new Set(normalize(heard).split(' ').filter(Boolean))
  if (said.length === 0) return 0
  return said.filter((word) => got.has(word)).length / said.length
}

test.describe('Homologação — o laço de conversa por voz', () => {
  test.beforeEach(async ({ window }) => {
    // Fixture boots with TTS off; here it's the measuring instrument.
    await window.evaluate(async () => {
      const api = (
        window as unknown as {
          electronAPI: { runtime: { saveSetting: (p: string, v: unknown) => Promise<unknown> } }
        }
      ).electronAPI.runtime
      await api.saveSetting('voice.ttsEnabled', true)
      await api.saveSetting('voice.ttsProvider', 'supertonic')
      await api.saveSetting('voice.asrProvider', 'electron')
      await api.saveSetting('voice.asrLanguage', 'pt')
      // Real production provider: homologating against the fake LLM proves nothing.
      await api.saveSetting('llm.provider', 'openrouter')
      await api.saveSetting('llm.openrouterModel', 'deepseek/deepseek-v4-flash-0731')
      await api.saveSetting('vision.provider', 'openrouter')
      await api.saveSetting('vision.model', 'google/gemini-2.5-flash-lite')
    })

    await window.evaluate(() => {
      const api = (
        window as unknown as {
          electronAPI: {
            runtime: {
              onTTSAudio: (cb: (p: { audio: string; mimeType: string }) => void) => void
              onToolCall: (cb: (p: { name: string; args: unknown }) => void) => void
            }
          }
        }
      ).electronAPI.runtime
      const captured: Array<{ audio: string; mimeType: string }> = []
      ;(window as unknown as { __ttsChunks: typeof captured }).__ttsChunks = captured
      api.onTTSAudio((payload) => captured.push({ audio: payload.audio, mimeType: payload.mimeType }))

      const tools: string[] = []
      ;(window as unknown as { __toolCalls: string[] }).__toolCalls = tools
      api.onToolCall((payload) => tools.push(payload.name))
    })
  })

  test('cinco frases: a Mika fala, o Whisper ouve de volta, e o LLM responde', async ({ window, runTool }) => {
    const results: LegResult[] = []

    for (const frase of PHRASES) {
      await window.evaluate(() => {
        ;(window as unknown as { __ttsChunks: TtsChunk[] }).__ttsChunks.length = 0
      })

      const spoken = await runTool('voice', { action: 'speak', text: frase })
      expect(spoken.success, `TTS falhou em "${frase}": ${spoken.error ?? ''}`).toBe(true)

      await expect
        .poll(
          async () => window.evaluate(() => (window as unknown as { __ttsChunks: TtsChunk[] }).__ttsChunks.length),
          { timeout: 60_000, message: `nenhum áudio de TTS chegou ao renderer para "${frase}"` },
        )
        .toBeGreaterThan(0)

      const heard = await transcribeCapturedAudio(window)

      // The real turn: the transcription is what she understood, becomes the message.
      await window.evaluate(() => {
        ;(window as unknown as { __toolCalls: string[] }).__toolCalls.length = 0
      })
      const turno = await window.evaluate(async (entendido) => {
        const api = (
          window as unknown as {
            electronAPI: {
              runtime: {
                sendChatMessage: (
                  text: string,
                  attachments?: unknown,
                  source?: string,
                ) => Promise<{ content?: string; error?: string }>
              }
            }
          }
        ).electronAPI.runtime
        const started = Date.now()
        const reply = await api.sendChatMessage(entendido, undefined, 'voice')
        return {
          conteudo: reply.content ?? '',
          erro: reply.error ?? '',
          ms: Date.now() - started,
          ferramentas: (window as unknown as { __toolCalls: string[] }).__toolCalls.slice(),
        }
      }, heard.text || frase)

      results.push({
        frase,
        ttsChunks: heard.chunks,
        audioMs: heard.audioMs,
        transcricao: heard.text,
        similaridade: similarity(frase, heard.text),
        rms: heard.rms,
        peak: heard.peak,
        resposta: turno.conteudo,
        respostaMs: turno.ms,
        ferramentas: turno.ferramentas,
        erro: turno.erro,
      })
    }

    writeReport(results)

    for (const leg of results) {
      expect(leg.audioMs, `o TTS não produziu áudio para "${leg.frase}"`).toBeGreaterThan(300)
      expect(
        leg.transcricao.length,
        `o Whisper devolveu vazio para "${leg.frase}" — o portão de fala barrou a voz da própria Mika`,
      ).toBeGreaterThan(0)
      expect(leg.similaridade, `transcrição não bate com "${leg.frase}": ouviu "${leg.transcricao}"`).toBeGreaterThan(
        0.5,
      )
      expect(leg.resposta, `a Mika não respondeu "${leg.frase}": ${leg.erro}`).not.toBe('')
    }
  })

  test('silêncio e ruído NÃO viram texto — é o turno fantasma que não pode voltar', async ({ window }) => {
    const veredito = await window.evaluate(async () => {
      const api = (
        window as unknown as {
          electronAPI: {
            runtime: {
              transcribeAudio: (p: {
                audioBase64: string
                mimeType: string
                language: string
              }) => Promise<{ success: boolean; data?: { text: string } }>
            }
          }
        }
      ).electronAPI.runtime

      const toBase64 = (pcm: Int16Array): string => {
        const bytes = new Uint8Array(pcm.buffer)
        let binary = ''
        for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i] as number)
        return btoa(binary)
      }

      // 5s each: the window where the old VAD held the recording and Whisper hallucinated text.
      const samples = 16_000 * 5
      const silence = new Int16Array(samples)
      const noise = new Int16Array(samples)
      for (let i = 0; i < samples; i++) noise[i] = Math.round((Math.random() * 2 - 1) * 6000)

      const ask = async (pcm: Int16Array) => {
        const result = await api.transcribeAudio({
          audioBase64: toBase64(pcm),
          mimeType: 'audio/pcm;rate=16000',
          language: 'pt',
        })
        return result.data?.text ?? ''
      }

      return { silencio: await ask(silence), ruido: await ask(noise) }
    })

    expect(veredito.silencio, `silêncio virou texto: "${veredito.silencio}"`).toBe('')
    expect(veredito.ruido, `ruído branco virou texto: "${veredito.ruido}"`).toBe('')
  })

  test('a resposta dela volta pelo mesmo caminho: LLM responde, TTS fala, Whisper transcreve', async ({
    window,
    runTool,
  }) => {
    const pergunta = 'Qual é a capital do Brasil?'

    const resposta = await window.evaluate(async (texto) => {
      const api = (
        window as unknown as {
          electronAPI: {
            runtime: {
              sendChatMessage: (
                text: string,
                attachments?: unknown,
                source?: string,
              ) => Promise<{ content?: string; error?: string }>
            }
          }
        }
      ).electronAPI.runtime
      return api.sendChatMessage(texto, undefined, 'voice')
    }, pergunta)

    expect(resposta.content ?? '', `o LLM não respondeu: ${resposta.error ?? 'sem conteúdo'}`).not.toBe('')

    await window.evaluate(() => {
      ;(window as unknown as { __ttsChunks: TtsChunk[] }).__ttsChunks.length = 0
    })

    const falado = (resposta.content ?? '').slice(0, 180)
    const spoken = await runTool('voice', { action: 'speak', text: falado })
    expect(spoken.success, `TTS falhou na resposta dela: ${spoken.error ?? ''}`).toBe(true)

    await expect
      .poll(async () => window.evaluate(() => (window as unknown as { __ttsChunks: TtsChunk[] }).__ttsChunks.length), {
        timeout: 60_000,
      })
      .toBeGreaterThan(0)

    const heard = await transcribeCapturedAudio(window)
    expect(heard.text.length, 'a fala da Mika não voltou pelo Whisper').toBeGreaterThan(0)

    writeFileSync(
      join(REPORT_DIR, 'voice-loop-resposta.md'),
      [
        '# Resposta da Mika, ida e volta',
        '',
        `- **Pergunta:** ${pergunta}`,
        `- **Resposta do LLM:** ${resposta.content}`,
        `- **Falado pelo TTS:** ${falado}`,
        `- **Ouvido de volta pelo Whisper:** ${heard.text}`,
        `- **Similaridade:** ${(similarity(falado, heard.text) * 100).toFixed(0)}%`,
        '',
      ].join('\n'),
      'utf8',
    )
  })
})

// Converts TTS audio to PCM16 mono 16kHz in the renderer — same path as real life; Main only accepts PCM16 16kHz.
async function transcribeCapturedAudio(
  window: import('@playwright/test').Page,
): Promise<{ text: string; chunks: number; audioMs: number; rms: number; peak: number }> {
  return window.evaluate(async () => {
    const chunks = (window as unknown as { __ttsChunks: Array<{ audio: string; mimeType: string }> }).__ttsChunks
    const api = (
      window as unknown as {
        electronAPI: {
          runtime: {
            transcribeAudio: (p: {
              audioBase64: string
              mimeType: string
              language: string
            }) => Promise<{ success: boolean; data?: { text: string }; error?: string }>
          }
        }
      }
    ).electronAPI.runtime

    const base64ToBytes = (base64: string): Uint8Array => {
      const binary = atob(base64)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
      return bytes
    }

    const decoded: Float32Array[] = []
    for (const chunk of chunks) {
      const ctx = new AudioContext()
      const buffer = await ctx.decodeAudioData(base64ToBytes(chunk.audio).buffer as ArrayBuffer)
      const offline = new OfflineAudioContext(1, Math.ceil((buffer.duration * 16_000) / 1) || 1, 16_000)
      const source = offline.createBufferSource()
      source.buffer = buffer
      source.connect(offline.destination)
      source.start()
      const resampled = await offline.startRendering()
      decoded.push(resampled.getChannelData(0).slice())
      await ctx.close()
    }

    const total = decoded.reduce((sum, part) => sum + part.length, 0)
    const merged = new Float32Array(total)
    let offset = 0
    for (const part of decoded) {
      merged.set(part, offset)
      offset += part.length
    }

    const pcm = new Int16Array(merged.length)
    for (let i = 0; i < merged.length; i++) {
      const sample = Math.max(-1, Math.min(1, merged[i] as number))
      pcm[i] = Math.round(sample * 32767)
    }

    const bytes = new Uint8Array(pcm.buffer)
    let binary = ''
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i] as number)

    const result = await api.transcribeAudio({
      audioBase64: btoa(binary),
      mimeType: 'audio/pcm;rate=16000',
      language: 'pt',
    })

    let energy = 0
    let peak = 0
    for (let i = 0; i < merged.length; i++) {
      const sample = Math.abs(merged[i] as number)
      energy += sample * sample
      if (sample > peak) peak = sample
    }

    return {
      text: result.data?.text ?? '',
      chunks: chunks.length,
      audioMs: Math.round((merged.length / 16_000) * 1000),
      rms: Number(Math.sqrt(energy / (merged.length || 1)).toFixed(4)),
      peak: Number(peak.toFixed(4)),
    }
  })
}

function writeReport(results: LegResult[]): void {
  mkdirSync(REPORT_DIR, { recursive: true })
  const lines = [
    '# Homologação do laço de voz',
    '',
    'A Mika fala a frase, o áudio dela volta pelo mesmo caminho do microfone e o Whisper transcreve.',
    '',
    '## O que ela ENTENDEU',
    '',
    '| Frase pedida | Ouvido de volta | Áudio | RMS | Similaridade |',
    '| --- | --- | ---: | ---: | ---: |',
    ...results.map(
      (leg) =>
        `| ${leg.frase} | ${leg.transcricao || '_(vazio)_'} | ${leg.audioMs} ms | ${leg.rms} | ${(leg.similaridade * 100).toFixed(0)}% |`,
    ),
    '',
    '## O que ela RESPONDEU, e o que ela FEZ',
    '',
    ...results.flatMap((leg) => [
      `### ${leg.frase}`,
      '',
      `- **Entendeu:** ${leg.transcricao || '_(vazio)_'}`,
      `- **Respondeu (${(leg.respostaMs / 1000).toFixed(1)}s):** ${leg.resposta || `_(vazio)_ ${leg.erro}`}`,
      `- **Ferramentas:** ${leg.ferramentas.length > 0 ? leg.ferramentas.join(', ') : '_(nenhuma)_'}`,
      '',
    ]),
  ]
  writeFileSync(join(REPORT_DIR, 'voice-loop.md'), lines.join('\n'), 'utf8')
}
