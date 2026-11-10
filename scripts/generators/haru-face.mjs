/**
 * Anime nose and mouth for the VRoid face. The base paints a 2-pixel pink nose tick and a pale lip
 * line that vanish under cel lighting, and its mouth interior is a flat salmon with no depth.
 */
import { decodePng, encodePng } from './haru-png.mjs'

const INK = [128, 58, 68]
const NOSE = [184, 112, 112]

function distanceToSegment(px, py, [ax, ay], [bx, by]) {
  const dx = bx - ax
  const dy = by - ay
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
  return Math.hypot(px - ax - t * dx, py - ay - t * dy)
}

/** Distance to the polyline and how far along it (0-1) the closest segment sits. */
function nearest(x, y, points) {
  let best = Number.POSITIVE_INFINITY
  let along = 0
  for (let i = 1; i < points.length; i++) {
    const d = distanceToSegment(x, y, points[i - 1], points[i])
    if (d < best) [best, along] = [d, (i - 0.5) / (points.length - 1)]
  }
  return [best, along]
}

/** Anti-aliased stroke along a polyline; `width(t)` tapers it from 0 to 1 along its length. */
function stroke({ width: w, rgba }, points, color, width, opacity = 1) {
  const xs = points.map(([x]) => x)
  const ys = points.map(([, y]) => y)
  const pad = Math.ceil(width(0.5)) + 2
  for (let y = Math.min(...ys) - pad; y <= Math.max(...ys) + pad; y++) {
    for (let x = Math.min(...xs) - pad; x <= Math.max(...xs) + pad; x++) {
      const [best, along] = nearest(x, y, points)
      const cover = Math.max(0, Math.min(1, width(along) / 2 + 0.5 - best)) * opacity
      if (cover <= 0) continue
      const o = (y * w + x) * 4
      for (let k = 0; k < 3; k++) rgba[o + k] += (color[k] - rgba[o + k]) * cover
    }
  }
}

function shade(image, cx, cy, rx, ry, amount) {
  for (let y = Math.floor(cy - ry); y <= cy + ry; y++) {
    for (let x = Math.floor(cx - rx); x <= cx + rx; x++) {
      const falloff = 1 - Math.hypot((x - cx) / rx, (y - cy) / ry)
      if (falloff <= 0) continue
      const o = (y * image.width + x) * 4
      for (let k = 0; k < 3; k++) image.rgba[o + k] *= 1 - amount * falloff ** 1.5 * [0.6, 1, 1][k]
    }
  }
}

/** Face_00 (1024²): nose tick at x 511, y 674-684; lip line at x 448-575, y 772-778. */
export function animeFacePng(png) {
  const image = decodePng(png)
  const taper = (base) => (t) => base * (0.35 + 0.65 * Math.sin(Math.PI * t))
  shade(image, 504, 684, 9, 12, 0.07)
  stroke(
    image,
    [
      [507, 668],
      [511, 678],
      [516, 687],
    ],
    NOSE,
    taper(3.2),
  )
  const smile = Array.from({ length: 13 }, (_, i) => {
    const x = 446 + i * 11
    return [x, 776 - 4 * ((x - 512) / 66) ** 2]
  })
  stroke(image, smile, INK, taper(3.4))
  return encodePng(image.width, image.height, image.rgba)
}

/** FaceMouth_00 (512²): teeth band top-left, tongue top-right, interior the bottom half. */
export function animeMouthPng(png) {
  const image = decodePng(png)
  const { width, height, rgba } = image
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4
      const lum = (rgba[o] + rgba[o + 1] + rgba[o + 2]) / 765
      let color
      // No white teeth band, like the Live2D Haru: closed, that band is the lip line that must read dark.
      if (y < height / 2 && x < width / 2) color = [150, 66, 76]
      else if (y < height / 2) color = [226 * lum + 20, 110 * lum + 18, 118 * lum + 20]
      else color = [110 + 70 * (y / height - 0.5), 34 + 24 * (y / height - 0.5), 48]
      rgba.set(
        color.map((c) => Math.round(Math.max(0, Math.min(255, c)))),
        o,
      )
    }
  }
  return encodePng(width, height, rgba)
}
