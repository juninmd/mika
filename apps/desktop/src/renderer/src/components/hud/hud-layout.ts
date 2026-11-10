/**
 * Anchors of the main-window HUD around the avatar, as pure math over the viewport.
 *
 * The face rect comes from the VRM head bone projected to the screen (`VrmCanvas` publishes it on
 * `lib/avatar-face-bus.ts` a couple of times per second) — the default model stands left of
 * center, and skins, poses and window sizes move it. Without a projection (Live2D, before the
 * first frame) it falls back to the band both backends frame her in (`avatar/avatar-hit-area.ts`).
 * Everything else is placed around the face; the face itself is a no-go zone for chrome.
 */
export type HudViewport = { width: number; height: number }
export type HudRect = { x: number; y: number; width: number; height: number }

export type BubbleAnchor = {
  /** `right`/`left`: beside her head, tail pointing at her mouth. `below`: under the chin, tail up. */
  side: 'right' | 'left' | 'below'
  /** Left edge (`right`/`left`) or horizontal center (`below`). */
  x: number
  y: number
  maxWidth: number
  maxHeight: number
  /** Side bubble: tail distance from the top. Below: tail distance from the bubble's center. */
  tailOffset: number
}

export type HudLayout = {
  face: HudRect
  bubble: BubbleAnchor
  /** Largest box the bubble may occupy — what the other anchors must stay clear of. */
  bubbleRect: HudRect
  /** The presence orb, on the side of her head opposite the bubble. */
  presence: HudRect
  gameChip: HudRect
  toasts: HudRect
  /** Bottom strip reserved for the dock and the composer that opens above it. */
  dockZone: number
}

const MARGIN = 12
const GAP = 12
const ORB = 36
const CHIP_HEIGHT = 30
const CHIP_MAX_WIDTH = 260
const TOAST_WIDTH = 288
const TOAST_MIN_HEIGHT = 96
const DOCK_ZONE = 88
const BUBBLE_MAX_WIDTH = 340
const BUBBLE_MIN_SIDE_WIDTH = 220
/** Where the tail sits on a side bubble: close to the top, so short replies still point at her. */
const SIDE_TAIL_FROM_TOP = 28
/** Hair and head width beyond the face, per side. */
const HEAD_TO_FACE = 1.7

// Fallback face band as fractions of the window height (eyes to chin), centered.
const FACE_TOP = 0.07
const FACE_HEIGHT = 0.16
const FACE_HALF_WIDTH = 0.065

const nonNegative = (v: number) => (Number.isFinite(v) && v > 0 ? v : 0)
const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi))

function bandFace(w: number, h: number): HudRect {
  const half = Math.min(FACE_HALF_WIDTH * h, 0.22 * w)
  return { x: w / 2 - half, y: FACE_TOP * h, width: half * 2, height: FACE_HEIGHT * h }
}

type Geometry = { w: number; h: number; face: HudRect; cx: number; head: number; topClear: number }

function sideBubble(g: Geometry, side: 'right' | 'left', width: number): BubbleAnchor {
  const mouthY = g.face.y + g.face.height * 0.75
  const y = Math.max(mouthY - SIDE_TAIL_FROM_TOP, g.topClear)
  const x = side === 'right' ? g.cx + g.head + GAP : g.cx - g.head - GAP - width
  const maxHeight = nonNegative(Math.min(0.4 * g.h, g.h - DOCK_ZONE - y - TOAST_MIN_HEIGHT - GAP))
  return { side, x, y, maxWidth: width, maxHeight, tailOffset: mouthY - y }
}

function bubbleAnchor(g: Geometry): BubbleAnchor {
  const right = Math.min(BUBBLE_MAX_WIDTH, g.w - MARGIN - (g.cx + g.head + GAP))
  if (right >= BUBBLE_MIN_SIDE_WIDTH) return sideBubble(g, 'right', right)
  const left = Math.min(BUBBLE_MAX_WIDTH, g.cx - g.head - GAP - MARGIN)
  if (left >= BUBBLE_MIN_SIDE_WIDTH) return sideBubble(g, 'left', left)
  const maxWidth = nonNegative(Math.min(BUBBLE_MAX_WIDTH, g.w - 2 * MARGIN))
  const y = g.face.y + g.face.height + GAP
  const x = clamp(g.cx, MARGIN + maxWidth / 2, g.w - MARGIN - maxWidth / 2)
  const maxHeight = nonNegative(Math.min(0.36 * g.h, g.h - DOCK_ZONE - y - TOAST_MIN_HEIGHT - GAP))
  return { side: 'below', x, y, maxWidth, maxHeight, tailOffset: g.cx - x }
}

function bubbleBox(b: BubbleAnchor): HudRect {
  const x = b.side === 'below' ? b.x - b.maxWidth / 2 : b.x
  return { x, y: b.y, width: b.maxWidth, height: b.maxHeight }
}

/** Top-left while it clears her face; otherwise beside the face, on whichever side has more room. */
function gameChipRect(w: number, face: HudRect): HudRect {
  if (MARGIN + CHIP_HEIGHT <= face.y) {
    return { x: MARGIN, y: MARGIN, width: nonNegative(Math.min(CHIP_MAX_WIDTH, w - 2 * MARGIN)), height: CHIP_HEIGHT }
  }
  const leftRoom = face.x - MARGIN - GAP
  const rightRoom = w - MARGIN - (face.x + face.width + GAP)
  const width = nonNegative(Math.min(CHIP_MAX_WIDTH, Math.max(leftRoom, rightRoom)))
  const x = leftRoom >= rightRoom ? MARGIN : w - MARGIN - width
  return { x, y: MARGIN, width, height: CHIP_HEIGHT }
}

function presenceRect(g: Geometry, bubble: BubbleAnchor): HudRect {
  const x =
    bubble.side === 'left'
      ? Math.min(g.w - MARGIN - ORB, g.cx + g.head + GAP)
      : Math.max(MARGIN, g.cx - g.head - GAP - ORB)
  const y = Math.max(g.face.y + g.face.height * 0.3, g.topClear)
  return { x, y, width: ORB, height: ORB }
}

export function computeHudLayout(viewport: HudViewport, projectedFace?: HudRect | null): HudLayout {
  const w = nonNegative(viewport.width)
  const h = nonNegative(viewport.height)
  const face = projectedFace ?? bandFace(w, h)
  const gameChip = gameChipRect(w, face)
  const g: Geometry = {
    w,
    h,
    face,
    cx: face.x + face.width / 2,
    head: (face.width / 2) * HEAD_TO_FACE,
    topClear: gameChip.y + gameChip.height + GAP,
  }
  const bubble = bubbleAnchor(g)
  const bubbleRect = bubbleBox(bubble)
  const presence = presenceRect(g, bubble)
  const toastTop = Math.max(bubbleRect.y + bubbleRect.height, face.y + face.height, presence.y + presence.height) + GAP
  const toastWidth = nonNegative(Math.min(TOAST_WIDTH, w - 2 * MARGIN))
  const toasts = {
    x: w - MARGIN - toastWidth,
    y: toastTop,
    width: toastWidth,
    height: nonNegative(h - DOCK_ZONE - toastTop),
  }
  return { face, bubble, bubbleRect, presence, gameChip, toasts, dockZone: DOCK_ZONE }
}
