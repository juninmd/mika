/**
 * Where her face is on screen, in CSS px — published by the avatar canvas a couple of times per
 * second (VRM head bone projected through the camera), read by the HUD layout. Outside React for
 * the same reason as `mic-level-bus`: the producer runs in the render loop.
 */
export type AvatarFace = { x: number; y: number; width: number; height: number }

const listeners = new Set<(face: AvatarFace | null) => void>()
let last: AvatarFace | null = null

/** Moves under this many px are head sway, not a new layout. */
const SETTLE_PX = 6

function moved(a: AvatarFace | null, b: AvatarFace | null): boolean {
  if (!a || !b) return a !== b
  return (
    Math.abs(a.x - b.x) > SETTLE_PX ||
    Math.abs(a.y - b.y) > SETTLE_PX ||
    Math.abs(a.width - b.width) > SETTLE_PX ||
    Math.abs(a.height - b.height) > SETTLE_PX
  )
}

export function publishAvatarFace(face: AvatarFace | null): void {
  if (!moved(last, face)) return
  last = face
  for (const listener of listeners) listener(face)
}

export function getAvatarFace(): AvatarFace | null {
  return last
}

export function subscribeAvatarFace(listener: (face: AvatarFace | null) => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
