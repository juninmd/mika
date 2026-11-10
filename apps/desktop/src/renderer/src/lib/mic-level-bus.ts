/** Nível do microfone, 10x/s, fora do React — `setState` nessa taxa no `useAppShell` re-renderiza a árvore inteira. */
type MicLevelSample = {
  /** 0..1 — pico normalizado da janela de áudio. */
  level: number
  /** O VAD passou do portão: isto é voz sendo capturada, não ruído de fundo. */
  hearing: boolean
}

const SILENT: MicLevelSample = { level: 0, hearing: false }

const listeners = new Set<(sample: MicLevelSample) => void>()
let last: MicLevelSample = SILENT

export function publishMicLevel(sample: MicLevelSample): void {
  last = sample
  for (const listener of listeners) listener(sample)
}

if (typeof window !== 'undefined') {
  ;(window as unknown as { __mikaMicBus?: { publish: typeof publishMicLevel; get: typeof getMicLevel } }).__mikaMicBus =
    {
      publish: publishMicLevel,
      get: getMicLevel,
    }
}

export function subscribeMicLevel(listener: (sample: MicLevelSample) => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function getMicLevel(): MicLevelSample {
  return last
}

/** Microfone desligado ou escuta parada: a onda tem de voltar ao zero, não congelar no último pico. */
export function resetMicLevel(): void {
  publishMicLevel(SILENT)
}
