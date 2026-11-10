export type EffectTier = 'full' | 'reduced' | 'eco'

export interface EffectScene {
  step(ctx: CanvasRenderingContext2D, dtMs: number, width: number, height: number): void
  readonly done: boolean
}

export const PARTICLE_SCALE: Record<EffectTier, number> = { full: 1, reduced: 0.55, eco: 0.3 }

export function rand(min: number, max: number): number {
  return min + Math.random() * (max - min)
}
