import { type EffectScene, type EffectTier, PARTICLE_SCALE, rand } from './scene-kit'

const FIREWORKS_TOTAL_MS = 9_000

export function createFireworksScene(tier: EffectTier): EffectScene {
  const scale = PARTICLE_SCALE[tier]
  const burstSize = Math.max(18, Math.round(64 * scale))
  type Particle = { x: number; y: number; vx: number; vy: number; life: number; maxLife: number; hue: number }
  type Rocket = { x: number; y: number; vy: number; hue: number }
  const rockets: Rocket[] = []
  const particles: Particle[] = []
  let elapsed = 0
  let nextLaunch = 300
  const PALETTE = [45, 180, 280, 330, 55]

  const explode = (x: number, y: number, hue: number) => {
    for (let i = 0; i < burstSize; i++) {
      const angle = (Math.PI * 2 * i) / burstSize + rand(-0.1, 0.1)
      const speed = rand(60, 240)
      particles.push({
        x,
        y,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        life: 0,
        maxLife: rand(900, 1700),
        hue: hue + rand(-18, 18),
      })
    }
  }

  return {
    get done() {
      return elapsed >= FIREWORKS_TOTAL_MS && particles.length === 0 && rockets.length === 0
    },
    step(ctx, dtMs, w, h) {
      ctx.clearRect(0, 0, w, h)
      const dt = Math.min(dtMs / 1000, 0.1)
      elapsed += dtMs
      nextLaunch -= dtMs
      if (nextLaunch <= 0 && elapsed < FIREWORKS_TOTAL_MS - 2_500) {
        nextLaunch = rand(500, 1100)
        rockets.push({
          x: rand(w * 0.15, w * 0.85),
          y: h,
          vy: rand(-h * 0.85, -h * 0.65),
          hue: PALETTE[Math.floor(Math.random() * PALETTE.length)],
        })
      }

      // Compactação no lugar: dois `filter` por frame alocavam arrays novos a 60 fps.
      let liveRockets = 0
      for (const r of rockets) {
        r.y += r.vy * dt
        r.vy += h * 0.35 * dt
        if (r.vy >= -30) {
          explode(r.x, r.y, r.hue)
          continue
        }
        ctx.fillStyle = `hsla(${r.hue}, 90%, 75%, 0.9)`
        ctx.beginPath()
        ctx.arc(r.x, r.y, 2.2, 0, Math.PI * 2)
        ctx.fill()
        rockets[liveRockets++] = r
      }
      rockets.length = liveRockets
      let liveParticles = 0
      for (const p of particles) {
        p.life += dtMs
        p.vy += 220 * dt
        p.vx *= 1 - 0.6 * dt
        p.vy *= 1 - 0.6 * dt
        p.x += p.vx * dt
        p.y += p.vy * dt
        const fade = 1 - p.life / p.maxLife
        if (fade <= 0) continue
        particles[liveParticles++] = p
        ctx.fillStyle = `hsla(${p.hue}, 95%, 65%, ${fade})`
        ctx.beginPath()
        ctx.arc(p.x, p.y, 2, 0, Math.PI * 2)
        ctx.fill()
      }
      particles.length = liveParticles
    },
  }
}
