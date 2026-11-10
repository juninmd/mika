import { memo } from 'react'

const STAR_COUNT = 16
const STAR_KEYS = Array.from({ length: STAR_COUNT }, (_, i) => `s-${i}`)

type Aurora = {
  left: string
  top: string
  size: string
  color: string
  opacity: number
  period: string
  reverse?: boolean
}

// Três auroras sobre base sólida: o degradê antigo partia de `--surface` (translúcido) e deixava o
// papel de parede vazar, daí o cinza lavado. Só `transform`/`opacity` animam (compositor) e o
// `prefers-reduced-motion` global já congela o loop.
const AURORAS: readonly Aurora[] = [
  { left: '18%', top: '22%', size: '46vmax', color: 'var(--accent)', opacity: 0.34, period: '28s' },
  { left: '84%', top: '16%', size: '40vmax', color: 'var(--neural-cyan)', opacity: 0.2, period: '36s', reverse: true },
  { left: '62%', top: '92%', size: '52vmax', color: 'var(--neural-purple)', opacity: 0.26, period: '32s' },
]

export const WizardBackdrop = memo(function WizardBackdrop() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 overflow-hidden"
      style={{ background: 'var(--bg)' }}
    >
      {AURORAS.map((aurora) => (
        <div
          key={aurora.color}
          className="stage-drift absolute rounded-full"
          style={{
            left: aurora.left,
            top: aurora.top,
            width: aurora.size,
            height: aurora.size,
            background: `radial-gradient(circle, ${aurora.color} 0%, transparent 62%)`,
            filter: 'blur(70px)',
            opacity: aurora.opacity,
            animationDirection: aurora.reverse ? 'reverse' : 'normal',
            ['--drift-period' as string]: aurora.period,
          }}
        />
      ))}

      {STAR_KEYS.map((key) => {
        const i = Number(key.split('-')[1])
        return (
          <div
            key={key}
            className="stage-twinkle absolute h-[2px] w-[2px] rounded-full"
            style={{
              left: `${6 + ((i * 17) % 88)}%`,
              top: `${8 + ((i * 29) % 84)}%`,
              background: i % 2 === 0 ? 'var(--accent)' : 'var(--info)',
              animationDelay: `${-(i * 0.9) % 6}s`,
              ['--twinkle-period' as string]: `${5 + (i % 4)}s`,
              ['--twinkle-lo' as string]: 0.12,
              ['--twinkle-hi' as string]: 0.55,
            }}
          />
        )
      })}

      <div
        className="absolute inset-0"
        style={{ background: 'radial-gradient(ellipse at center, transparent 45%, var(--bg) 100%)', opacity: 0.55 }}
      />
    </div>
  )
})
