// VRM mouth: audio vowels → visemes with fast attack and slower release.
import { VOWEL_PRESET } from './vrm-look'

export interface VowelWeightsLike {
  a: number
  i: number
  u: number
  e: number
  o: number
}

export type VisemeName = (typeof VOWEL_PRESET)[keyof typeof VOWEL_PRESET]
export type Visemes = Record<VisemeName, number>

/** Idle-state cap: analyser noise must not move a closed mouth. */
export const IDLE_OPEN_CAP = 0.15
const ATTACK_RATE = 30
const RELEASE_RATE = 13

const VOWELS = Object.entries(VOWEL_PRESET) as Array<[keyof VowelWeightsLike, VisemeName]>

export const silentVisemes = (): Visemes => ({ aa: 0, ih: 0, ou: 0, ee: 0, oh: 0 })

/** Each viseme gets its slice of the openness; with no vowel signal, raw openness goes to `aa`. */
export function visemeTargets(vowels: VowelWeightsLike, openness: number, speaking: boolean): Visemes {
  const open = speaking ? openness : Math.min(openness, IDLE_OPEN_CAP)
  const total = vowels.a + vowels.i + vowels.u + vowels.e + vowels.o
  const out = silentVisemes()
  for (const [vowel, viseme] of VOWELS) {
    out[viseme] = total > 0 ? (vowels[vowel] / total) * open : vowel === 'a' ? open : 0
  }
  return out
}

// Raw values swapped vowel every frame and made the mouth tremble; fast attack + slow release smooths it.
export class VisemeSmoother {
  readonly current = silentVisemes()

  advance(target: Visemes, dt: number): Visemes {
    const attack = 1 - Math.exp(-dt * ATTACK_RATE)
    const release = 1 - Math.exp(-dt * RELEASE_RATE)
    for (const [, viseme] of VOWELS) {
      const from = this.current[viseme]
      const to = Number.isFinite(target[viseme]) ? target[viseme] : 0
      this.current[viseme] = from + (to - from) * (to > from ? attack : release)
    }
    return this.current
  }
}
