import { EFFECT_QUALITY_MAX_FPS, type EffectQuality } from '../../lib/effect-quality'

export interface VrmFramePolicy {
  paused: boolean
  minFrameMs: number
  idle: boolean
}

// Quality only scales the frame rate; idle poses stay on whenever she is drawn at all.
export function vrmFramePolicy(quality: EffectQuality): VrmFramePolicy {
  if (quality === 'paused') return { paused: true, minFrameMs: Number.POSITIVE_INFINITY, idle: false }
  return { paused: false, minFrameMs: 1000 / EFFECT_QUALITY_MAX_FPS[quality], idle: true }
}

/** Folga para o jitter do vsync: 16.66 ms não pode perder para um teto de 16.67 ms. */
const VSYNC_SLACK_MS = 2

/** Teto de quadros com cadência fixa: o resto do intervalo fica no relógio em vez de ser descartado. */
export function gateFrame(now: number, last: number, minFrameMs: number): { render: boolean; last: number } {
  const elapsed = now - last
  if (elapsed < minFrameMs - VSYNC_SLACK_MS) return { render: false, last }
  // Depois de uma pausa ou engasgo, ressincroniza em vez de disparar os quadros atrasados em rajada.
  return { render: true, last: elapsed > minFrameMs * 2 ? now : last + minFrameMs }
}
