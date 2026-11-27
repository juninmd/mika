import { describe, expect, it } from 'vitest'
import {
  decideEndOfTurn,
  type EndOfTurnCheck,
  lastSmartTurnWindow,
  SMART_TURN_MAX_SILENCE_MS,
  SMART_TURN_WINDOW_BYTES,
} from '../speech-end-of-turn'
import { MAX_UTTERANCE_MS } from '../speech-transcribe'

const HANGOVER = 1_200
const MAX_SILENCE = Math.max(SMART_TURN_MAX_SILENCE_MS, HANGOVER)
const VOICE_AT = 10_000

const idle: EndOfTurnCheck = { status: 'idle' }
const pending: EndOfTurnCheck = { status: 'pending', voiceAt: VOICE_AT }
const complete: EndOfTurnCheck = { status: 'complete', voiceAt: VOICE_AT, probability: 0.9, latencyMs: 12 }
const incomplete: EndOfTurnCheck = { status: 'incomplete', voiceAt: VOICE_AT, probability: 0.1, latencyMs: 12 }
const failed: EndOfTurnCheck = { status: 'error', voiceAt: VOICE_AT }

function decide(overrides: Partial<Parameters<typeof decideEndOfTurn>[0]> = {}) {
  return decideEndOfTurn({
    silentForMs: HANGOVER,
    elapsedMs: 5_000,
    hangoverMs: HANGOVER,
    maxSilenceMs: MAX_SILENCE,
    endpointingEnabled: true,
    smartTurnEnabled: true,
    lastVoiceAt: VOICE_AT,
    check: idle,
    ...overrides,
  })
}

describe('decideEndOfTurn — Smart Turn ligado', () => {
  it('completo encerra já no hangover', () => {
    expect(decide({ check: complete })).toBe('finish')
  })

  it('incompleto continua escutando até o teto de segurança', () => {
    expect(decide({ check: incomplete })).toBe('wait')
    expect(decide({ check: incomplete, silentForMs: MAX_SILENCE - 1 })).toBe('wait')
    expect(decide({ check: incomplete, silentForMs: MAX_SILENCE })).toBe('finish')
  })

  it('antes do hangover espera, sem pedir classificação', () => {
    expect(decide({ silentForMs: HANGOVER - 1 })).toBe('wait')
  })

  it('no hangover sem resultado pede UMA classificação', () => {
    expect(decide({ check: idle })).toBe('check')
  })

  it('enquanto a classificação está pendente não dispara outra', () => {
    expect(decide({ check: pending })).toBe('wait')
    expect(decide({ check: pending, silentForMs: MAX_SILENCE - 1 })).toBe('wait')
  })

  it('erro ou timeout do classificador encerra pelo VAD', () => {
    expect(decide({ check: failed })).toBe('finish')
  })

  it('voz retomada invalida o resultado da pausa anterior', () => {
    const resumedAt = VOICE_AT + 500
    expect(decide({ lastVoiceAt: resumedAt, check: complete })).toBe('check')
    expect(decide({ lastVoiceAt: resumedAt, check: incomplete })).toBe('check')
    // A nova pausa ainda está no começo: espera antes de checar.
    expect(decide({ lastVoiceAt: resumedAt, silentForMs: HANGOVER - 1, check: complete })).toBe('wait')
  })

  it('o teto acompanha um hangover maior que 3 s', () => {
    const longerHangover = 4_000
    const max = Math.max(SMART_TURN_MAX_SILENCE_MS, longerHangover)
    expect(decide({ hangoverMs: longerHangover, maxSilenceMs: max, silentForMs: 3_500, check: incomplete })).toBe(
      'wait',
    )
    expect(decide({ hangoverMs: longerHangover, maxSilenceMs: max, silentForMs: max, check: incomplete })).toBe(
      'finish',
    )
  })

  it('o teto de 3 s vence uma classificação ainda pendente', () => {
    expect(decide({ check: pending, silentForMs: MAX_SILENCE })).toBe('finish')
  })
})

describe('decideEndOfTurn — comportamento antigo preservado', () => {
  it('Smart Turn desligado encerra exatamente no hangover, ignorando o resultado', () => {
    expect(decide({ smartTurnEnabled: false, silentForMs: HANGOVER - 1, check: complete })).toBe('wait')
    expect(decide({ smartTurnEnabled: false, silentForMs: HANGOVER, check: complete })).toBe('finish')
    expect(decide({ smartTurnEnabled: false, silentForMs: HANGOVER, check: incomplete })).toBe('finish')
  })

  it('endpointing desligado só encerra no teto de 45 s', () => {
    expect(decide({ endpointingEnabled: false, silentForMs: 999_999 })).toBe('wait')
    expect(decide({ endpointingEnabled: false, silentForMs: 999_999, elapsedMs: MAX_UTTERANCE_MS })).toBe('finish')
  })

  it('o teto MAX_UTTERANCE_MS continua valendo com Smart Turn ligado e classificação incompleta', () => {
    expect(decide({ elapsedMs: MAX_UTTERANCE_MS, check: incomplete })).toBe('finish')
    expect(decide({ elapsedMs: MAX_UTTERANCE_MS, check: pending })).toBe('finish')
  })
})

describe('lastSmartTurnWindow — só os últimos 8 s de PCM16', () => {
  it('áudio maior que 8 s mantém o fim', () => {
    const pcm = Uint8Array.from({ length: SMART_TURN_WINDOW_BYTES + 4 }, (_, i) => i % 251)
    const window = lastSmartTurnWindow(pcm)
    expect(window).toHaveLength(SMART_TURN_WINDOW_BYTES)
    expect(window[0]).toBe(pcm[4])
    expect(window.at(-1)).toBe(pcm.at(-1))
  })

  it('áudio curto passa inteiro', () => {
    const pcm = Uint8Array.from([1, 2, 3, 4])
    expect(lastSmartTurnWindow(pcm)).toBe(pcm)
  })
})
