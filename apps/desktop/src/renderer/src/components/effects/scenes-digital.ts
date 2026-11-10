import { type EffectScene, type EffectTier, PARTICLE_SCALE, rand } from './scene-kit'

const MATRIX_GLYPHS = 'ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃ0123456789<>/:;=+*'
const MATRIX_CELL = 18
const MATRIX_TAIL = 8

type Drop = { col: number; y: number; speed: number; glyphs: string[] }

export function createMatrixScene(tier: EffectTier): EffectScene {
  const density = 0.55 * PARTICLE_SCALE[tier]
  let drops: Drop[] = []
  let width = 0
  let height = 0

  const newGlyphs = () =>
    Array.from({ length: MATRIX_TAIL }, () => MATRIX_GLYPHS[Math.floor(Math.random() * MATRIX_GLYPHS.length)])

  const seed = (w: number, h: number) => {
    width = w
    height = h
    const cols = Math.max(1, Math.floor((w / MATRIX_CELL) * density))
    drops = Array.from({ length: cols }, (_, col) => ({
      col,

      y: rand(-h * 0.2, h),
      speed: rand(120, 420),
      glyphs: newGlyphs(),
    }))
  }

  return {
    get done() {
      return false
    },
    step(ctx, dtMs, w, h) {
      if (w !== width || h !== height || drops.length === 0) seed(w, h)
      ctx.clearRect(0, 0, w, h)
      const dt = Math.min(dtMs / 1000, 0.1)
      ctx.font = '13px monospace'
      ctx.textAlign = 'center'
      for (const drop of drops) {
        drop.y += drop.speed * dt
        if (drop.y - MATRIX_TAIL * MATRIX_CELL > h) {
          drop.y = rand(-h * 0.5, 0)
          drop.speed = rand(120, 420)
          drop.glyphs = newGlyphs()
          continue
        }
        drawMatrixDrop(ctx, drop, h)
      }
    },
  }
}

function drawMatrixDrop(ctx: CanvasRenderingContext2D, drop: Drop, h: number): void {
  const x = drop.col * MATRIX_CELL + MATRIX_CELL / 2
  for (let i = 0; i < MATRIX_TAIL; i++) {
    const y = drop.y - i * MATRIX_CELL
    if (y < 0 || y > h) continue
    const alpha = i === 0 ? 0.95 : 0.55 * (1 - i / MATRIX_TAIL)
    ctx.fillStyle = i === 0 ? `rgba(190, 255, 200, ${alpha})` : `rgba(34, 197, 94, ${alpha})`
    ctx.fillText(drop.glyphs[i], x, y)
  }
}

type Star = { x: number; y: number; vx: number; vy: number; r: number; phase: number }

export function createConstellationScene(tier: EffectTier): EffectScene {
  const count = Math.max(12, Math.round(70 * PARTICLE_SCALE[tier]))
  let stars: Star[] = []
  let width = 0
  let height = 0
  const LINK_DIST = 130

  return {
    get done() {
      return false
    },
    step(ctx, dtMs, w, h) {
      if (w !== width || h !== height || stars.length === 0) {
        width = w
        height = h
        stars = Array.from({ length: count }, () => ({
          x: rand(0, w),
          y: rand(0, h),
          vx: rand(-12, 12),
          vy: rand(-12, 12),
          r: rand(0.8, 2.4),
          phase: rand(0, Math.PI * 2),
        }))
      }
      ctx.clearRect(0, 0, w, h)
      const dt = Math.min(dtMs / 1000, 0.1)
      moveStars(stars, dt, w, h)

      ctx.lineWidth = 1
      drawStarLinks(ctx, stars, LINK_DIST)
      drawStars(ctx, stars)
    },
  }
}

function moveStars(stars: Star[], dt: number, w: number, h: number): void {
  for (const s of stars) {
    s.x += s.vx * dt
    s.y += s.vy * dt
    s.phase += dt * 2
    if (s.x < 0 || s.x > w) s.vx *= -1
    if (s.y < 0 || s.y > h) s.vy *= -1
  }
}

function drawStarLinks(ctx: CanvasRenderingContext2D, stars: Star[], linkDist: number): void {
  for (let i = 0; i < stars.length; i++) {
    for (let j = i + 1; j < stars.length; j++) {
      const dist = Math.hypot(stars[i].x - stars[j].x, stars[i].y - stars[j].y)
      if (dist >= linkDist) continue
      ctx.strokeStyle = `rgba(139, 92, 246, ${0.35 * (1 - dist / linkDist)})`
      ctx.beginPath()
      ctx.moveTo(stars[i].x, stars[i].y)
      ctx.lineTo(stars[j].x, stars[j].y)
      ctx.stroke()
    }
  }
}

function drawStars(ctx: CanvasRenderingContext2D, stars: Star[]): void {
  for (const s of stars) {
    const twinkle = 0.6 + 0.4 * Math.sin(s.phase)
    ctx.fillStyle = `rgba(255, 255, 255, ${0.85 * twinkle})`
    ctx.beginPath()
    ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2)
    ctx.fill()
  }
}
