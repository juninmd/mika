import { memo } from 'react'
import { useEffectQuality } from '../../hooks/useEffectQuality'
import type { AppStatus } from './app-shell.types'

type AuraTone = { color: string; lo: number; hi: number; period: number }

/** Cor = o que ela está fazendo; ritmo e brilho sobem com a atividade. `offline` não brilha. */
const AURA: Record<AppStatus, AuraTone | null> = {
  idle: { color: 'var(--accent)', lo: 0.14, hi: 0.3, period: 6 },
  listening: { color: 'var(--ok)', lo: 0.22, hi: 0.5, period: 2.6 },
  thinking: { color: 'var(--info)', lo: 0.22, hi: 0.5, period: 1.8 },
  acting: { color: 'var(--warn)', lo: 0.2, hi: 0.44, period: 2.2 },
  confirming: { color: 'var(--warn)', lo: 0.26, hi: 0.5, period: 3.2 },
  speaking: { color: 'var(--accent)', lo: 0.28, hi: 0.6, period: 1.4 },
  error: { color: 'var(--danger)', lo: 0.2, hi: 0.4, period: 3 },
  offline: null,
}

/**
 * Halo atrás da avatar que respira conforme o estado. Fica sob o canvas (não cobre o rosto) e só
 * anima `transform`/`opacity`; com o app pausado/em segundo plano (`useEffectQuality`) some.
 */
export const AvatarAura = memo(function AvatarAura({ status }: { status: AppStatus }) {
  const quality = useEffectQuality(status === 'speaking')
  const tone = AURA[status]
  if (!tone || quality === 'paused') return null

  return (
    <div
      aria-hidden="true"
      data-testid="avatar-aura"
      data-status={status}
      className="aura-breathe pointer-events-none absolute left-1/2 top-[46%] rounded-full"
      style={
        {
          width: 'min(62vh, 70vw)',
          height: 'min(62vh, 70vw)',
          background: `radial-gradient(closest-side, ${tone.color}, transparent 72%)`,
          filter: 'blur(28px)',
          '--aura-lo': tone.lo,
          '--aura-hi': tone.hi,
          '--aura-period': `${tone.period}s`,
        } as React.CSSProperties
      }
    />
  )
})
