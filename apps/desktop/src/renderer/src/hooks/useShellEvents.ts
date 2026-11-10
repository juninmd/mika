import type { VTuberExpression } from '@mika/sdk/contracts'
import { useEffect, useState } from 'react'
import { getElectronApi } from '../lib/shell-api'

type Params = {
  platform: string
  spawnReaction: (x: number, y: number) => void
  setExpression: (expr: VTuberExpression, intensity?: number) => void
  scheduleExpressionReset: (fn: () => void, ms: number) => void
}

/** Miscellaneous shell-wide effects: OS class toggle, Live2D readiness delay, and screen-observation reactions. */
export function useShellEvents({ platform, spawnReaction, setExpression, scheduleExpressionReset }: Params) {
  const [live2dReady, setLive2dReady] = useState(false)

  useEffect(() => {
    if (platform === 'linux') document.body.classList.add('is-linux')
    return () => {
      document.body.classList.remove('is-linux')
    }
  }, [platform])

  useEffect(() => {
    const timer = setTimeout(() => setLive2dReady(true), 250)
    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI.runtime.onScreenObservation) return
    const unsub = eAPI.runtime.onScreenObservation((payload) => {
      if (!payload.isFunny) return
      setExpression('excited')
      const cx = window.innerWidth / 2
      const cy = window.innerHeight * 0.4
      spawnReaction(cx, cy)
      scheduleExpressionReset(() => setExpression('neutral'), 3000)
    })
    return () => {
      unsub?.()
    }
  }, [spawnReaction, setExpression, scheduleExpressionReset])

  return { live2dReady }
}
