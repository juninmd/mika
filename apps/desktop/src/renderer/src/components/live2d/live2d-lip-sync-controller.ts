import type { VowelWeights } from '../../lib/lip-sync-vowels'
import { getLipSyncTarget } from './live2d-lip-sync'
import type { Live2DModelInstance } from './live2d-loader'
import { EnvelopeFollower, type PerlinNoise1D } from './live2d-physics'

interface CoreModel {
  setParameterValueById(id: string, value: number): void
}

const MOUTH_ATTACK_MS = 18
const MOUTH_RELEASE_MS = 90

const VOWEL_ATTACK_MS = 45
const VOWEL_RELEASE_MS = 110

const VOWEL_ACTIVE_THRESHOLD = 0.02

export type VowelParams = Partial<Record<keyof VowelWeights, string>>

export class Live2DLipSyncController {
  private readonly envelope = new EnvelopeFollower(MOUTH_ATTACK_MS, MOUTH_RELEASE_MS)
  private readonly vowelEnvelopes: Record<keyof VowelWeights, EnvelopeFollower> = {
    a: new EnvelopeFollower(VOWEL_ATTACK_MS, VOWEL_RELEASE_MS),
    i: new EnvelopeFollower(VOWEL_ATTACK_MS, VOWEL_RELEASE_MS),
    u: new EnvelopeFollower(VOWEL_ATTACK_MS, VOWEL_RELEASE_MS),
    e: new EnvelopeFollower(VOWEL_ATTACK_MS, VOWEL_RELEASE_MS),
    o: new EnvelopeFollower(VOWEL_ATTACK_MS, VOWEL_RELEASE_MS),
  }

  get value(): number {
    return this.envelope.value
  }

  update({
    mouthFormParam,
    mouthOpenness,
    volume,
    vowels,
    vowelParams,
    expression,
    speaking,
    time,
    dt,
    noise,
    model,
  }: {
    mouthOpenness: number
    volume: number | undefined
    expression: string
    speaking: boolean
    time: number
    dt: number
    noise: PerlinNoise1D
    model: Live2DModelInstance

    mouthFormParam?: string | null

    vowels?: VowelWeights | null

    vowelParams?: VowelParams
  }): number {
    const smoothed = this.envelope.update(getLipSyncTarget(mouthOpenness, volume, expression), dt)
    const coreModel = model.internalModel.coreModel as unknown as CoreModel

    const hasVowelParams = vowelParams !== undefined && Object.keys(vowelParams).length > 0
    if (hasVowelParams) {
      this.applyVowels(coreModel, vowelParams, vowels ?? null, smoothed, dt)
    }

    if (mouthFormParam) {
      const mouthForm = hasVowelParams
        ? this.formFromVowels()
        : speaking && smoothed > 0.05
          ? (noise.get(time * 10) + 1) * 0.5 - 0.5
          : 0
      coreModel.setParameterValueById(mouthFormParam, mouthForm)
    }

    return smoothed
  }

  private applyVowels(
    coreModel: CoreModel,
    vowelParams: VowelParams,
    vowels: VowelWeights | null,
    openness: number,
    dt: number,
  ): void {
    for (const [vowel, envelope] of Object.entries(this.vowelEnvelopes) as Array<
      [keyof VowelWeights, EnvelopeFollower]
    >) {
      const target = (vowels?.[vowel] ?? 0) * openness
      const value = envelope.update(target, dt)
      const param = vowelParams[vowel]
      if (param && value > VOWEL_ACTIVE_THRESHOLD) coreModel.setParameterValueById(param, Math.min(1, value))
      else if (param) coreModel.setParameterValueById(param, 0)
    }
  }

  private formFromVowels(): number {
    const wide = this.vowelEnvelopes.i.value + this.vowelEnvelopes.e.value
    const round = this.vowelEnvelopes.u.value + this.vowelEnvelopes.o.value
    return Math.max(-1, Math.min(1, wide - round))
  }
}
