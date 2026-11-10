import { type EffectScene, type EffectTier, PARTICLE_SCALE, rand } from './scene-kit'

export function createSakuraScene(tier: EffectTier): EffectScene {
  const count = Math.max(8, Math.round(42 * PARTICLE_SCALE[tier]))
  type Petal = {
    x: number
    y: number
    fall: number
    sway: number
    phase: number
    size: number
    spin: number
    rot: number
  }
  let petals: Petal[] = []
  let width = 0
  let height = 0

  return {
    get done() {
      return false
    },
    step(ctx, dtMs, w, h) {
      if (w !== width || h !== height || petals.length === 0) {
        width = w
        height = h
        petals = Array.from({ length: count }, () => ({
          x: rand(0, w),
          y: rand(-h, 0),
          fall: rand(45, 110),
          sway: rand(20, 60),
          phase: rand(0, Math.PI * 2),
          size: rand(4, 9),
          spin: rand(-1.4, 1.4),
          rot: rand(0, Math.PI * 2),
        }))
      }
      ctx.clearRect(0, 0, w, h)
      const dt = Math.min(dtMs / 1000, 0.1)
      for (const p of petals) {
        p.phase += dt * 1.6
        p.rot += p.spin * dt
        p.y += p.fall * dt
        const x = p.x + Math.sin(p.phase) * p.sway
        if (p.y - p.size > h) {
          p.y = rand(-40, 0)
          p.x = rand(0, w)
          continue
        }
        ctx.save()
        ctx.translate(x, p.y)
        ctx.rotate(p.rot + Math.sin(p.phase) * 0.6)
        ctx.fillStyle = `rgba(249, 168, 212, ${0.55 + 0.3 * Math.abs(Math.sin(p.phase))})`
        ctx.beginPath()

        ctx.ellipse(0, 0, p.size, p.size * 0.55, 0, 0, Math.PI * 2)
        ctx.fill()
        ctx.restore()
      }
    },
  }
}

export function createSynthwaveScene(tier: EffectTier): EffectScene {
  const HORIZON = 0.62
  const GRID_COLS = 12
  const GRID_ROWS = 10
  let scroll = 0

  return {
    get done() {
      return false
    },
    step(ctx, dtMs, w, h) {
      ctx.clearRect(0, 0, w, h)
      const horizonY = h * HORIZON
      const vpx = w / 2

      const sunR = Math.min(w, h) * 0.16
      const sunGrad = ctx.createLinearGradient(0, horizonY - sunR * 2, 0, horizonY)
      sunGrad.addColorStop(0, 'rgba(244, 114, 182, 0.8)')
      sunGrad.addColorStop(1, 'rgba(251, 146, 60, 0.55)')
      ctx.fillStyle = sunGrad
      ctx.beginPath()
      ctx.arc(vpx, horizonY, sunR, Math.PI, Math.PI * 2)
      ctx.fill()

      ctx.lineWidth = 1
      for (let i = -GRID_COLS; i <= GRID_COLS; i++) {
        const xBase = vpx + (i / GRID_COLS) * w * 0.9
        ctx.strokeStyle = 'rgba(168, 85, 247, 0.4)'
        ctx.beginPath()
        ctx.moveTo(vpx, horizonY)
        ctx.lineTo(xBase, h)
        ctx.stroke()
      }

      scroll = (scroll + dtMs * 0.0012) % 1
      const skip = tier === 'full' ? 1 : 2
      for (let r = 0; r < GRID_ROWS; r++) {
        if (r % skip !== 0) continue
        const t = (r / GRID_ROWS + scroll) % 1
        const y = horizonY + t ** 2.6 * (h - horizonY)
        if (y > h) continue
        ctx.strokeStyle = `rgba(34, 211, 238, ${0.15 + 0.45 * t})`
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(w, y)
        ctx.stroke()
      }
    },
  }
}
