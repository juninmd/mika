export type EffectQuality = 'full' | 'reduced' | 'eco' | 'paused'

interface EffectQualityInput {
  hidden: boolean

  focused: boolean

  onBattery: boolean

  speaking?: boolean

  reducedMotion?: boolean

  /** Teto do governador de energia (Rust `modules/system/governor`): jogo/ocioso/Economia baixam o fps. */
  ceiling?: EffectCeiling
}

export type EffectCeiling = Exclude<EffectQuality, 'paused'>

const RANK: Record<EffectQuality, number> = { paused: 0, eco: 1, reduced: 2, full: 3 }

function resolveUncapped(input: EffectQualityInput): EffectQuality {
  if (input.hidden) return 'paused'
  if (input.reducedMotion) return input.speaking ? 'reduced' : 'paused'
  if (input.speaking || input.focused) return input.onBattery ? 'reduced' : 'full'
  return 'eco'
}

// Arrasto nativo da janela: o SO move o frame e o webview recompõe a cada passo; renderizar o avatar a
// 60 fps nesse instante só disputa CPU/GPU com a composição. Teto de 'reduced' (30 fps) enquanto dura.
let windowDragging = false
const dragListeners = new Set<() => void>()

export function setWindowDragging(value: boolean): void {
  if (windowDragging === value) return
  windowDragging = value
  for (const notify of dragListeners) notify()
}

export function subscribeWindowDragging(notify: () => void): () => void {
  dragListeners.add(notify)
  return () => dragListeners.delete(notify)
}

export function resolveEffectQuality(input: EffectQualityInput): EffectQuality {
  const quality = resolveUncapped(input)
  const ceiling = windowDragging && RANK[input.ceiling ?? 'full'] > RANK.reduced ? 'reduced' : (input.ceiling ?? 'full')
  return RANK[quality] > RANK[ceiling] ? ceiling : quality
}

export function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
    : false
}

export const EFFECT_QUALITY_MAX_FPS: Record<Exclude<EffectQuality, 'paused'>, number> = {
  full: 60,
  reduced: 30,
  eco: 15,
}
