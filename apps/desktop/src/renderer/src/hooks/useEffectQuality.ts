import { useEffect, useState } from 'react'
import { type EffectQuality, prefersReducedMotion, resolveEffectQuality } from '../lib/effect-quality'
import { getPowerSnapshot, subscribePower } from '../lib/power-profile'

function currentQuality(speaking: boolean): EffectQuality {
  return resolveEffectQuality({
    hidden: document.hidden,
    focused: document.hasFocus(),
    onBattery: getPowerSnapshot().onBattery,
    ceiling: getPowerSnapshot().ceiling,
    reducedMotion: prefersReducedMotion(),
    speaking,
  })
}

export function useEffectQuality(speaking = false): EffectQuality {
  const [quality, setQuality] = useState<EffectQuality>(() => currentQuality(speaking))

  useEffect(() => {
    const update = () => setQuality(currentQuality(speaking))
    update()

    document.addEventListener('visibilitychange', update)
    window.addEventListener('focus', update)
    window.addEventListener('blur', update)
    const reducedMotionQuery =
      typeof window.matchMedia === 'function' ? window.matchMedia('(prefers-reduced-motion: reduce)') : null
    reducedMotionQuery?.addEventListener?.('change', update)
    const unsubscribe = subscribePower(update)

    return () => {
      document.removeEventListener('visibilitychange', update)
      window.removeEventListener('focus', update)
      window.removeEventListener('blur', update)
      reducedMotionQuery?.removeEventListener?.('change', update)
      unsubscribe()
    }
  }, [speaking])

  return quality
}
