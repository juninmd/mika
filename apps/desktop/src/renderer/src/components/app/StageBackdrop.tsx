import { memo } from 'react'

type StageBackdropProps = {
  backgroundMode: 'transparent' | 'semi-transparent' | 'solid'
  backgroundSolidColor: string
}

const PARTICLE_COUNT = 24
const PARTICLE_KEYS = Array.from({ length: PARTICLE_COUNT }, (_, i) => `p-${i}`)

export const StageBackdrop = memo(function StageBackdrop({ backgroundMode, backgroundSolidColor }: StageBackdropProps) {
  if (backgroundMode === 'transparent') return null

  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0" style={{ zIndex: 'var(--layer-ambient)' }}>
      <div
        data-testid="stage-backdrop-fill"
        className="absolute inset-0 transition-opacity duration-1000"
        style={{
          opacity: backgroundMode === 'solid' ? 1 : 0.35,
          background: `linear-gradient(180deg, ${backgroundMode === 'solid' ? backgroundSolidColor : 'transparent'}, var(--bg))`,
        }}
      />

      <div className="absolute inset-0">
        <div
          className="stage-drift absolute left-1/2 top-[38%] h-[60vh] w-[70vw]"
          style={{
            background:
              'radial-gradient(ellipse at var(--nebula-x, 40%) var(--nebula-y, 45%), var(--neural-cyan) 0%, transparent 60%)',
            filter: 'blur(80px)',
            opacity: 0.12,
            transformOrigin: 'center center',
          }}
        />
        <div
          className="stage-drift absolute left-1/2 top-[42%] h-[50vh] w-[60vw]"
          style={{
            animationDirection: 'reverse',
            ['--drift-period' as string]: '34s',
            background:
              'radial-gradient(ellipse at calc(100% - var(--nebula-x, 40%)) calc(100% - var(--nebula-y, 45%)), var(--neural-purple) 0%, transparent 55%)',
            filter: 'blur(90px)',
            opacity: 0.1,
            transformOrigin: 'center center',
          }}
        />
      </div>

      <div className="absolute left-1/2 top-[44%] h-[30vh] w-[30vw] -translate-x-1/2 -translate-y-1/2">
        <div
          className="absolute inset-0 rounded-full"
          style={{
            background: 'radial-gradient(circle, var(--primary) 0%, transparent 70%)',
            opacity: 0.06,
          }}
        />
        <div
          className="absolute inset-[15%] rounded-full"
          style={{
            background: 'radial-gradient(circle, var(--neural-cyan) 0%, transparent 70%)',
            opacity: 0.04,
            animationDelay: '2s',
          }}
        />
      </div>

      <div className="absolute inset-0 overflow-hidden">
        {PARTICLE_KEYS.map((key) => {
          const i = Number(key.split('-')[1])
          return (
            <div
              key={key}
              className="stage-twinkle absolute h-[2px] w-[2px] rounded-full"
              style={{
                animationDelay: `${-(i * 0.83) % 6}s`,
                ['--twinkle-period' as string]: `${5 + (i % 5)}s`,
                ['--twinkle-lo' as string]: 0.1 + (i % 4) * 0.04,
                ['--twinkle-hi' as string]: 0.4 + (i % 3) * 0.12,
                left: `${5 + ((i * 7) % 90)}%`,
                top: `${15 + ((i * 13) % 75)}%`,
                background: i % 3 === 0 ? 'var(--info)' : i % 3 === 1 ? 'var(--accent)' : 'var(--ok)',
                boxShadow: `0 0 6px currentColor`,
              }}
            />
          )
        })}
      </div>

      <div
        className="absolute bottom-[-8%] left-1/2 h-[20vh] w-[90vw] -translate-x-1/2"
        style={{
          background: `radial-gradient(ellipse at center bottom, var(--primary) 0%, transparent 70%)`,
          filter: 'blur(40px)',
          opacity: 0.03,
        }}
      />

      <div
        className="absolute inset-x-0 top-1/2 h-px"
        style={{
          background:
            'repeating-linear-gradient(90deg, transparent, var(--neural-cyan) 1px, transparent 2px, transparent 40px)',
          backgroundSize: '200% 100%',
          filter: 'blur(0.5px)',
          opacity: 0.015,
        }}
      />
    </div>
  )
})
