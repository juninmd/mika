/**
 * Drawn cloth folds for the tailored torso, as a grayscale multiply over the look's tint: anime
 * tops read as fabric through tension lines from the bust to the waist and bunching at the sides.
 * The torso is a cylinder UV: u runs around from the front centre, v from the hem (0.855) up.
 */
const [HEM, HEIGHT, RADIUS] = [0.855, 0.49, 0.13]
const TURN = 2 * Math.PI * RADIUS

// Polylines in (around, height): `around` in turns from the front centre, height in metres.
const TENSION = [
  [
    [0.115, 1.165],
    [0.08, 1.09],
    [0.04, 1.025],
  ],
]
const SIDE_BUNCH = [1.018, 1.043, 1.068].map((y) => [
  [0.17, y + 0.004],
  [0.2, y - 0.004],
  [0.23, y + 0.004],
])
const BACK = [0.975, 1.005, 1.035].map((y) => [
  [0.44, y],
  [0.5, y - 0.006],
  [0.56, y],
])
const FOLDS = [...TENSION, ...SIDE_BUNCH].flatMap((line) => [line, line.map(([a, y]) => [-a, y])]).concat(BACK)

function distanceToSegment(px, py, [ax, ay], [bx, by]) {
  const dx = bx - ax
  const dy = by - ay
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
  return Math.hypot(px - ax - t * dx, py - ay - t * dy)
}

/** Metric distance from a torso point to the nearest fold; `around` wraps at the back. */
function foldDistance(around, y, lift = 0) {
  let best = Number.POSITIVE_INFINITY
  for (const line of FOLDS) {
    for (let i = 1; i < line.length; i++) {
      const [a0, y0] = line[i - 1]
      const [a1, y1] = line[i]
      const wrap = Math.round(around - (a0 + a1) / 2)
      const d = distanceToSegment((around - wrap) * TURN, y, [a0 * TURN, y0 + lift], [a1 * TURN, y1 + lift])
      best = Math.min(best, d)
    }
  }
  return best
}

export function foldColor(u, v) {
  const y = HEM + v * HEIGHT
  const around = u > 0.5 ? u - 1 : u
  const front = Math.abs(around) < 0.14 ? 1 : 0
  const underBust = Math.exp(-(((y - 1.172) / 0.012) ** 2)) * front * 0.1
  const crease = Math.exp(-((foldDistance(around, y) / 0.0036) ** 2)) * 0.22
  const sheen = Math.exp(-((foldDistance(around, y, 0.007) / 0.004) ** 2)) * 0.06
  const weave = Math.sin(u * Math.PI * 1024) * Math.sin(v * Math.PI * 512) * 3
  const value = 240 * (1 - crease - underBust) + 255 * sheen + weave
  return [value, value, value * 1.01 + 2, 255]
}
