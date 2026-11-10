import { describe, expect, it } from 'vitest'
import {
  decideSpokenTurn,
  HELD_SPEECH_TTL_MS,
  hasDegenerateRepetition,
  isHeldSpeechStale,
  isImplausibleTranscript,
} from '../speech-turn'

describe('decideSpokenTurn', () => {
  const fala = { text: 'que você me sugere jogar?', durationMs: 2400 }

  it('guarda a fala quando um turno está em andamento, em vez de jogar fora', () => {
    expect(decideSpokenTurn({ ...fala, busy: true })).toBe('hold')
  })

  it('manda direto quando não há turno em voo', () => {
    expect(decideSpokenTurn({ ...fala, busy: false })).toBe('send')
  })

  it('ruído de uma letra não vira turno, nem ocupado nem livre', () => {
    expect(decideSpokenTurn({ text: 'e', durationMs: 900, busy: false })).toBe('too-short')
    expect(decideSpokenTurn({ text: 'e', durationMs: 900, busy: true })).toBe('too-short')
  })

  it('alucinação não vira turno nem entra na fila — nem ocupada, nem livre', () => {
    const fantasma = { text: 'E aí', durationMs: 20_342 }
    expect(decideSpokenTurn({ ...fantasma, busy: false })).toBe('implausible')
    expect(decideSpokenTurn({ ...fantasma, busy: true })).toBe('implausible')
  })
})

/** Casos medidos em produção: áudio longo virando poucos caracteres é NÃO-FALA (ruído alto), não silêncio. */
describe('isImplausibleTranscript', () => {
  it.each([
    [10_640, 'e'],
    [20_342, 'E aí'],
    [11_101, 'E aí'],
    [11_492, 'Tchau!'],
    [6801, 'E aí'],
    [9501, 'O que é isso?'],
  ])('%dms de áudio para %o é alucinação, não fala', (durationMs, text) => {
    expect(isImplausibleTranscript(text, durationMs)).toBe(true)
  })

  it.each([
    [5400, 'Você poderia parar de ficar falando toda hora disso, é isso?'],
    [3601, 'Qual é a capital do Brasil?'],
    [4700, 'Qual é o melhor sushi que existe?'],
    [6202, 'Acho melhor sushi clássico.'],
    [6600, 'Você poderia parar de ficar falando de CS toda hora?'],
  ])('%dms para %o é fala de verdade e passa', (durationMs, text) => {
    expect(isImplausibleTranscript(text, durationMs)).toBe(false)
  })

  it('áudio curto demais não é julgado por densidade — dois caracteres a mais mudariam o veredito', () => {
    expect(isImplausibleTranscript('E aí', 600)).toBe(false)
  })

  // A regressão ao vivo: o usuário falou "E aí" e a Mika ficou surda. Os 3,5 s de janela para 4
  // caracteres dão 1,1 c/s — abaixo do piso de 2 —, mas são a fala curta MAIS o hangover de
  // silêncio, não um fantasma. A densidade baixa só volta a valer para texto longo.
  it('fala curta de verdade com o hangover de silêncio não é descartada', () => {
    expect(isImplausibleTranscript('E aí', 3529)).toBe(false)
  })

  // Abaixo do piso de duração do fantasma, a frase de estoque é indistinguível de alguém que falou
  // pouco. Este é o preço explícito de não descartar mais o "E aí" de verdade: estes três casos
  // passaram a ser aceitos, e quem os barra adiante é o mínimo de caracteres do turno.
  it.each([
    [3902, 'O que é isso?'],
    [4201, 'E aí'],
    [2000, 'Tchau!'],
  ])('%dms para a frase de estoque %o é curto demais para julgar e passa', (durationMs, text) => {
    expect(isImplausibleTranscript(text, durationMs)).toBe(false)
  })

  it('frase de estoque dita de verdade, no ritmo de quem fala, passa', () => {
    // "E aí" em meio segundo = 8 c/s; abaixo do teto de medição, nem chega a ser julgada.
    expect(isImplausibleTranscript('E aí', 500)).toBe(false)
    // 25 caracteres em 8,9 s = 2,8 c/s: acima do piso de densidade e longe de frases de estoque.
    expect(isImplausibleTranscript('Amanhã não vou trabalhar.', 8935)).toBe(false)
  })

  it('sem a janela de áudio (reconhecimento do navegador) não se julga', () => {
    expect(isImplausibleTranscript('E aí', undefined)).toBe(false)
  })
})

describe('hasDegenerateRepetition', () => {
  it('pega o laço travado do decoder', () => {
    expect(hasDegenerateRepetition('E aí E aí E aí E aí E aí E aí E aí E aí')).toBe(true)
    expect(hasDegenerateRepetition('um vídeo de um vídeo de um vídeo de um vídeo de um vídeo de')).toBe(true)
  })

  it('não confunde repetição natural com laço', () => {
    expect(hasDegenerateRepetition('eu quero muito muito muito jogar isso hoje à noite')).toBe(false)
    expect(hasDegenerateRepetition('E aí')).toBe(false)
  })
})

describe('isHeldSpeechStale', () => {
  it('fala recém-guardada continua valendo', () => {
    expect(isHeldSpeechStale(1_000, 1_000 + HELD_SPEECH_TTL_MS - 1)).toBe(false)
  })

  it('passou do teto, o assunto já morreu — responder soaria pior que calar', () => {
    expect(isHeldSpeechStale(1_000, 1_000 + HELD_SPEECH_TTL_MS + 1)).toBe(true)
  })
})
