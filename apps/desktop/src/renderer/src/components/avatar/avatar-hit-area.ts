// Pure hit-area test shared by both avatar backends: does a click land on the avatar's head?
// Both Live2D (`fitModelToStage`/STAGE_HEIGHT_FILL) and VRM (its camera framing) anchor the model
// bottom-center and fill nearly the whole canvas height, so the head sits in a stable band near the
// top of the canvas regardless of backend or loaded skin — no per-model bounding box needed.

export interface AvatarClickPoint {
  /** Fraction of the canvas width, 0 (left) to 1 (right). */
  xFrac: number
  /** Fraction of the canvas height, 0 (top) to 1 (bottom). */
  yFrac: number
}

const HEAD_X_MIN = 0.3
const HEAD_X_MAX = 0.7
const HEAD_Y_MIN = 0.04
const HEAD_Y_MAX = 0.24

export function isHeadHitArea({ xFrac, yFrac }: AvatarClickPoint): boolean {
  return xFrac >= HEAD_X_MIN && xFrac <= HEAD_X_MAX && yFrac >= HEAD_Y_MIN && yFrac <= HEAD_Y_MAX
}
