import { describe, expect, it } from 'vitest'
import { hasDegenerateRepetition, isImplausibleTranscript } from '../../src/utils/transcript-plausibility'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0x78a5c)

const WHISPER_FILLERS = [
  'e ai',
  'o que e isso',
  'tchau',
  'obrigado',
  'obrigada',
  'ate mais',
  'legendas pela comunidade amara org',
  'valeu',
  'ah',
  'oi',
]

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyz'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(random() * chars.length))
  }
  return result
}

function randomWord(): string {
  return randomString(Math.floor(random() * 8) + 3).trim() || 'word'
}

function generateNormalText(minWords = 10, maxWords = 50): string {
  const count = Math.floor(random() * (maxWords - minWords)) + minWords
  const words = []
  for (let i = 0; i < count; i++) {
    words.push(randomWord())
  }
  return words.join(' ')
}

function generateDegenerateText(unitSize: 1 | 2 | 3): string {
  const unit = Array.from({ length: unitSize }, () => randomWord()).join(' ')
  // needs to be at least 8 words long, and repeat 4 times
  const minRepeats = Math.max(4, Math.ceil(8 / unitSize))
  const repeats = Math.floor(random() * 5) + minRepeats
  return Array.from({ length: repeats }, () => unit).join(' ')
}

describe('Transcript Plausibility - Fuzzy Tests', () => {
  describe('hasDegenerateRepetition', () => {
    it('should return false for normal, varied random texts', () => {
      for (let i = 0; i < 1000; i++) {
        const text = generateNormalText()
        expect(hasDegenerateRepetition(text)).toBe(false)
      }
    })

    it('should return true for degenerate repetition texts of unit size 1', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateDegenerateText(1)
        expect(hasDegenerateRepetition(text)).toBe(true)
      }
    })

    it('should return true for degenerate repetition texts of unit size 2', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateDegenerateText(2)
        expect(hasDegenerateRepetition(text)).toBe(true)
      }
    })

    it('should return true for degenerate repetition texts of unit size 3', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateDegenerateText(3)
        expect(hasDegenerateRepetition(text)).toBe(true)
      }
    })
  })

  describe('isImplausibleTranscript', () => {
    it('should return false if durationMs is omitted or less than MIN_MEASURABLE_MS', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateNormalText()
        // No duration
        expect(isImplausibleTranscript(text)).toBe(false)

        // Short duration
        const durationMs = Math.floor(random() * 1199) // < 1200
        expect(isImplausibleTranscript(text, durationMs)).toBe(false)
      }
    })

    it('should return false for normal text with valid density (2-22 c/s)', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateNormalText(10, 30) // ~50-150 chars
        const minDuration = (text.length / 22) * 1000 // Density 22
        const maxDuration = (text.length / 2) * 1000 // Density 2

        // Ensure duration is >= 1200
        let durationMs = random() * (maxDuration - minDuration) + minDuration
        if (durationMs < 1200) durationMs = 1200 + random() * 1000

        // Re-check density after adjusting duration
        const density = (text.length / durationMs) * 1000
        if (density >= 2 && density <= 22) {
          expect(isImplausibleTranscript(text, durationMs)).toBe(false)
        }
      }
    })

    it('should return true for texts with implausibly low density (< 2 c/s)', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateNormalText(5, 10) // ~25-50 chars
        // To get density < 2, duration > (text.length / 2) * 1000
        const durationMs = (text.length / 2) * 1000 + 1000 + random() * 10000

        if (durationMs >= 1200) {
          expect(isImplausibleTranscript(text, durationMs)).toBe(true)
        }
      }
    })

    it('should return true for texts with implausibly high density (> 22 c/s)', () => {
      for (let i = 0; i < 500; i++) {
        const text = generateNormalText(50, 100) // ~250-500 chars
        // To get density > 22, duration < (text.length / 22) * 1000
        let durationMs = (text.length / 22) * 1000 - 100 - random() * 500

        // Ensure duration is >= 1200 for the check to trigger
        if (durationMs < 1200) durationMs = 1200

        const density = (text.length / durationMs) * 1000
        if (density > 22) {
          expect(isImplausibleTranscript(text, durationMs)).toBe(true)
        }
      }
    })

    it('should flag known fillers as implausible if density < 6 c/s and the window is too long for short speech', () => {
      for (let i = 0; i < 500; i++) {
        const filler = WHISPER_FILLERS[Math.floor(random() * WHISPER_FILLERS.length)]
        // To get density < 6, duration > (filler.length / 6) * 1000
        const minDurationForLowDensity = (filler.length / 6) * 1000 + 1
        // E acima do piso de 6000 ms: abaixo dele a frase de estoque é indistinguível de quem falou
        // pouco, porque a fala curta real também gera uma janela de ~3,5 s (fala + hangover).
        const durationMs = Math.max(6000, minDurationForLowDensity) + random() * 5000

        expect(isImplausibleTranscript(filler, durationMs)).toBe(true)
      }
    })

    it('should NOT flag known fillers as implausible if density >= 6 c/s and density <= 22 (and duration >= 1200ms)', () => {
      for (let i = 0; i < 500; i++) {
        const filler = WHISPER_FILLERS[Math.floor(random() * WHISPER_FILLERS.length)]
        // Density >= 6 means duration <= (filler.length / 6) * 1000
        // Density <= 22 means duration >= (filler.length / 22) * 1000
        const minDuration = (filler.length / 22) * 1000
        const maxDuration = (filler.length / 6) * 1000

        // Generate duration within [max(1200, minDuration), maxDuration]
        const startRange = Math.max(1200, minDuration)
        if (startRange <= maxDuration) {
          const durationMs = startRange + random() * (maxDuration - startRange)
          expect(isImplausibleTranscript(filler, durationMs)).toBe(false)
        }
      }
    })
  })
})
