const F1_LOW_HZ = 250
const F1_HIGH_HZ = 1000

const F2_LOW_HZ = 900
const F2_HIGH_HZ = 3000

const SILENCE_DBFS = -55
const LOUD_DBFS = -12

const VOWEL_MIN_LOUDNESS = 0.12

export interface VowelWeights {
  a: number
  i: number
  u: number
  e: number
  o: number
}

export const SILENT_VOWELS: VowelWeights = { a: 0, i: 0, u: 0, e: 0, o: 0 }

const VOWEL_FORMANTS: Record<keyof VowelWeights, readonly [f1: number, f2: number]> = {
  a: [750, 1300],

  i: [300, 2400],

  u: [350, 800],
  e: [500, 1900],
  o: [500, 950],
}

export function rmsLoudness(timeDomain: Float32Array): number {
  if (timeDomain.length === 0) return 0
  let sum = 0
  // Acesso direto por índice: typed array carregado do range contrato do analyser; sem branch
  // nullish por amostra (minor, mas o hot path roda ~60x/s por quadro de fala).
  for (let i = 0; i < timeDomain.length; i++) {
    sum += timeDomain[i] * timeDomain[i]
  }
  return Math.sqrt(sum / timeDomain.length)
}

export function loudnessToOpenness(rms: number): number {
  if (rms <= 0) return 0
  const dbfs = 20 * Math.log10(rms)
  if (dbfs <= SILENCE_DBFS) return 0
  if (dbfs >= LOUD_DBFS) return 1
  return (dbfs - SILENCE_DBFS) / (LOUD_DBFS - SILENCE_DBFS)
}

function peakHzInRange(freqData: Uint8Array, binSize: number, lowHz: number, highHz: number): number {
  const lowBin = Math.max(1, Math.floor(lowHz / binSize))
  const highBin = Math.min(Math.floor(highHz / binSize), freqData.length - 1)
  if (highBin < lowBin) return 0

  let peakBin = lowBin
  let peakValue = -1
  for (let bin = lowBin; bin <= highBin; bin++) {
    // Índices garantidos dentro do range: highBin é fechado contra freqData.length - 1.
    const value = freqData[bin]
    if (value > peakValue) {
      peakValue = value
      peakBin = bin
    }
  }

  return peakValue < 8 ? 0 : peakBin * binSize
}

export interface Formants {
  f1: number
  f2: number
}

export function estimateFormants(freqData: Uint8Array, sampleRate: number, fftSize: number): Formants {
  const binSize = sampleRate / fftSize
  if (!Number.isFinite(binSize) || binSize <= 0) return { f1: 0, f2: 0 }
  return {
    f1: peakHzInRange(freqData, binSize, F1_LOW_HZ, F1_HIGH_HZ),
    f2: peakHzInRange(freqData, binSize, F2_LOW_HZ, F2_HIGH_HZ),
  }
}

const toMel = (hz: number) => 2595 * Math.log10(1 + hz / 700)

/** Centros das vogais já em mel: `vowelWeights` roda a cada frame enquanto a Mika fala. */
const VOWEL_CENTERS_MEL: ReadonlyArray<readonly [vowel: keyof VowelWeights, f1: number, f2: number]> = (
  Object.entries(VOWEL_FORMANTS) as Array<[keyof VowelWeights, readonly [number, number]]>
).map(([vowel, [f1, f2]]) => [vowel, toMel(f1), toMel(f2)])

/** Rascunho de por-vogal reutilizado entre chamadas — `vowelWeights` roda ~60x/s e não
 *  acumula lixo a cada quadro (o array de scores e as tuplas de antes eram lixo por frame). */
const SCORES = new Float64Array(VOWEL_CENTERS_MEL.length)

export function vowelWeights(formants: Formants, loudness: number): VowelWeights {
  if (loudness < VOWEL_MIN_LOUDNESS || formants.f1 <= 0 || formants.f2 <= 0) return { ...SILENT_VOWELS }

  const f1Mel = toMel(formants.f1)
  const f2Mel = toMel(formants.f2)

  let total = 0
  for (let k = 0; k < VOWEL_CENTERS_MEL.length; k++) {
    const center = VOWEL_CENTERS_MEL[k] as readonly [keyof VowelWeights, number, number]
    const d1 = f1Mel - center[1]
    const d2 = f2Mel - center[2]
    const distance = Math.sqrt(d1 * d1 + d2 * d2)

    const score = 1 / (1 + (distance / 100) ** 2)
    SCORES[k] = score
    total += score
  }

  const weights: VowelWeights = { ...SILENT_VOWELS }
  for (let k = 0; k < VOWEL_CENTERS_MEL.length; k++) {
    const center = VOWEL_CENTERS_MEL[k] as readonly [keyof VowelWeights, number, number]
    weights[center[0]] = SCORES[k] / total
  }
  return weights
}

export function dominantVowel(weights: VowelWeights): { vowel: keyof VowelWeights; weight: number } {
  let best: keyof VowelWeights = 'a'
  let bestWeight = -1
  for (const [vowel, weight] of Object.entries(weights) as Array<[keyof VowelWeights, number]>) {
    if (weight > bestWeight) {
      bestWeight = weight
      best = vowel
    }
  }
  return { vowel: best, weight: bestWeight }
}
