import { describe, expect, it } from 'vitest'
import { computeHudLayout, type HudRect } from '../hud-layout'

const overlap = (a: HudRect, b: HudRect) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height

// Real sizes: the avatar window opens at 400×640 (min 300×400, `src-tauri/src/windows.rs`), and a
// user may stretch it to a desktop-wide stage.
const VIEWPORTS = [
  [300, 400],
  [400, 640],
  [520, 720],
  [800, 600],
  [1280, 860],
  [1920, 1080],
] as const

describe('computeHudLayout', () => {
  it('narrow avatar window: the bubble sits under the chin, tail up, never over the face', () => {
    const l = computeHudLayout({ width: 400, height: 640 })
    expect(l.bubble.side).toBe('below')
    expect(l.bubble.y).toBeGreaterThanOrEqual(l.face.y + l.face.height)
  })

  it('wide stage: the bubble moves beside her head with the tail pointing at her mouth', () => {
    const l = computeHudLayout({ width: 1280, height: 860 })
    expect(l.bubble.side).toBe('right')
    expect(l.bubble.x).toBeGreaterThan(l.face.x + l.face.width)
    const mouthY = l.face.y + l.face.height * 0.75
    expect(l.bubble.y + l.bubble.tailOffset).toBeCloseTo(mouthY, 0)
  })

  it.each(VIEWPORTS)('%i×%i: presence, game chip, bubble and toasts never cover her face', (width, height) => {
    const l = computeHudLayout({ width, height })
    for (const rect of [l.presence, l.gameChip, l.bubbleRect, l.toasts]) expect(overlap(rect, l.face)).toBe(false)
  })

  it.each(VIEWPORTS)('%i×%i: the anchored elements do not collide with each other', (width, height) => {
    const l = computeHudLayout({ width, height })
    const rects = [l.presence, l.gameChip, l.bubbleRect, l.toasts]
    for (let i = 0; i < rects.length; i++)
      for (let j = i + 1; j < rects.length; j++) expect(overlap(rects[i] as HudRect, rects[j] as HudRect)).toBe(false)
  })

  it.each(VIEWPORTS)('%i×%i: everything stays inside the window, clear of the dock zone', (width, height) => {
    const l = computeHudLayout({ width, height })
    for (const r of [l.presence, l.gameChip, l.bubbleRect, l.toasts]) {
      expect(r.x).toBeGreaterThanOrEqual(0)
      expect(r.y).toBeGreaterThanOrEqual(0)
      expect(r.x + r.width).toBeLessThanOrEqual(width)
      expect(r.y + r.height).toBeLessThanOrEqual(height - l.dockZone)
    }
  })

  // The real face comes from projecting the VRM head bone (`VrmCanvas` → `avatar-face-bus`); the
  // default model stands left of center, a moved/rescaled one can be anywhere.
  it('follows the projected face instead of assuming the window center', () => {
    const face = { x: 90, y: 60, width: 80, height: 90 }
    const l = computeHudLayout({ width: 520, height: 640 }, face)
    expect(l.face).toEqual(face)
    expect(l.bubble.side).toBe('right')
    expect(l.bubble.x).toBeGreaterThan(face.x + face.width)
  })

  it('a face near the right edge sends the bubble to her left, tail pointing right, presence to the other side', () => {
    const face = { x: 900, y: 80, width: 110, height: 130 }
    const l = computeHudLayout({ width: 1100, height: 800 }, face)
    expect(l.bubble.side).toBe('left')
    expect(l.bubble.x).toBeLessThan(face.x)
    expect(l.presence.x).toBeGreaterThan(face.x + face.width)
  })

  const FACES: Array<[number, number, HudRect]> = [
    [400, 640, { x: 88, y: 58, width: 84, height: 96 }],
    [400, 640, { x: 150, y: 50, width: 100, height: 110 }],
    [1280, 860, { x: 500, y: 70, width: 100, height: 120 }],
    [1280, 860, { x: 1050, y: 70, width: 100, height: 120 }],
    [520, 720, { x: 330, y: 60, width: 90, height: 100 }],
  ]
  it.each(FACES)('%i×%i with an off-center face: nothing covers it and nothing collides', (width, height, face) => {
    const l = computeHudLayout({ width, height }, face)
    const rects = [l.presence, l.gameChip, l.bubbleRect, l.toasts]
    for (const rect of rects) expect(overlap(rect, face)).toBe(false)
    for (let i = 0; i < rects.length; i++)
      for (let j = i + 1; j < rects.length; j++) expect(overlap(rects[i] as HudRect, rects[j] as HudRect)).toBe(false)
  })

  it('the bubble never grows past a readable measure, even on a 4K stage', () => {
    expect(computeHudLayout({ width: 3840, height: 2160 }).bubble.maxWidth).toBeLessThanOrEqual(360)
  })

  it('degenerate sizes (hidden window reports 0×0) do not produce NaN anchors', () => {
    const l = computeHudLayout({ width: 0, height: 0 })
    for (const v of Object.values(l.bubble)) if (typeof v === 'number') expect(Number.isFinite(v)).toBe(true)
  })
})
