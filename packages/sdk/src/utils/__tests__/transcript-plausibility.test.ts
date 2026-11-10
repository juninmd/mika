import { describe, expect, it } from 'vitest'
import { hasDegenerateRepetition, isImplausibleTranscript } from '../transcript-plausibility'

/**
 * Filters `whisper-small` returning stock phrases on non-speech (34 of 51 transcripts in one real session).
 * Density-only judging failed on short real speech ("E aí" over the 3.5s silence hangover window is below
 * the density floor), so these cases check intent, not raw output: real short speech passes, 10-20s ghosts don't.
 */
describe('isImplausibleTranscript', () => {
  it('fala curta real com a janela de silêncio não é descartada', () => {
    // Literal log regression: "E aí" (4 chars) in 3529ms is speech + silence, not stretched characters.
    expect(isImplausibleTranscript('E aí', 3529)).toBe(false)
  })

  it.each([
    [20_342, 'E aí'],
    [11_492, 'Tchau!'],
    [10_640, 'e'],
  ])('%dms de áudio para %o é fantasma: 10–20 s não viram poucos caracteres', (durationMs, text) => {
    expect(isImplausibleTranscript(text, durationMs)).toBe(true)
  })

  it.each([
    [3601, 'Qual é a capital do Brasil?'],
    [5400, 'Você poderia parar de ficar falando toda hora disso, é isso?'],
    [6202, 'Acho melhor sushi clássico.'],
  ])('%dms para %o é fala no ritmo humano e passa', (durationMs, text) => {
    expect(isImplausibleTranscript(text, durationMs)).toBe(false)
  })

  it('texto longo com densidade baixa em áudio longo condena — aí a razão mede algo', () => {
    // 23 chars in 20s = 1.15 c/s; unlike the short "E aí" case, this density is meaningful.
    expect(isImplausibleTranscript('eu não falei isso agora', 20_000)).toBe(true)
  })

  it('texto impossível para a janela condena pelo teto superior, sem depender do tamanho', () => {
    expect(isImplausibleTranscript('a'.repeat(300), 3000)).toBe(true)
  })

  it('frase de estoque curta demais não é condenada — indistinguível de quem falou pouco', () => {
    // Trade-off of keeping real short "E aí": below the duration floor, "Tchau!" at 2s also passes.
    expect(isImplausibleTranscript('Tchau!', 2000)).toBe(false)
  })

  it('o laço degenerado do decoder condena sempre, antes de qualquer conta de densidade', () => {
    expect(isImplausibleTranscript('um vídeo de um vídeo de um vídeo de um vídeo de', 3529)).toBe(true)
  })

  it('sem a janela de áudio (reconhecimento do navegador) não se julga', () => {
    expect(isImplausibleTranscript('E aí', undefined)).toBe(false)
  })
})

describe('hasDegenerateRepetition', () => {
  it('pega o laço travado do decoder', () => {
    expect(hasDegenerateRepetition('um vídeo de um vídeo de um vídeo de um vídeo de')).toBe(true)
  })

  it('não confunde repetição natural nem fala curta com laço', () => {
    expect(hasDegenerateRepetition('E aí')).toBe(false)
    expect(hasDegenerateRepetition('eu quero muito muito muito jogar isso hoje à noite')).toBe(false)
  })
})
