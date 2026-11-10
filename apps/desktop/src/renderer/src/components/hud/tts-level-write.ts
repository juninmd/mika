/** Escrita de nível de TTS no medidor do orb: quantizada a 3 casas — só muda se o valor legível mudou. */
export function ttsLevelWrite(next: number, last: number): number {
  const q = Math.round(Math.min(1, Math.max(0, next)) * 1000) / 1000
  return q === last ? last : q
}
