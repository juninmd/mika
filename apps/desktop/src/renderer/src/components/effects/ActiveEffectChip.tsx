import { Sparkles } from 'lucide-react'
import { memo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { matchEffectLabel } from '../../lib/match-effect'
import { useEffectsStore } from '../../store/useEffectsStore'

export const ActiveEffectChip = memo(function ActiveEffectChip() {
  const effect = useEffectsStore((s) => s.activeEffect)
  const applyMatchEffect = useEffectsStore((s) => s.applyMatchEffect)
  const { t } = useI18n()

  if (!effect) return null
  const label = matchEffectLabel(effect, t)

  return (
    <button
      type="button"
      onClick={() => applyMatchEffect({ effect, mode: 'toggle' })}
      aria-pressed="true"
      className="pointer-events-auto flex items-center gap-2 rounded-full border border-accent-rim/40 bg-bg-surface/90 px-3 py-1.5 text-caption font-semibold text-text-main backdrop-blur-md shadow-xs shadow-accent/15 transition active:scale-95 hover:border-accent-rim hover:bg-surface-raise-strong focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      aria-label={t('shell.effects.chip.aria', { effect: label })}
    >
      <Sparkles aria-hidden="true" size={14} />
      <span>{label}</span>
      <span className="text-text-muted/90 font-mono font-medium">· /{effect}</span>
    </button>
  )
})
