import { useEffect, useRef } from 'react'
import { rendererLog } from '../lib/renderer-logger'

export function useMouseTracking(enabled: boolean, onTrack: (x: number, y: number) => void) {
  const lastTimeRef = useRef(0)
  const droppedRef = useRef(0)
  const lastWarnAtRef = useRef(0)

  useEffect(() => {
    if (!enabled) return

    const handleMouseMove = (e: MouseEvent) => {
      const now = performance.now()
      if (now - lastTimeRef.current < 66) {
        droppedRef.current++
        if (droppedRef.current >= 120 && now - lastWarnAtRef.current > 5000) {
          lastWarnAtRef.current = now

          rendererLog.debug('[Desktop Shell] mouse tracking rate clamped', {
            rateHz: 15,
            droppedSamples: droppedRef.current,
          })
          droppedRef.current = 0
        }
        return
      }
      lastTimeRef.current = now

      const centerX = window.innerWidth / 2
      const centerY = window.innerHeight / 2

      const nx = (e.clientX - centerX) / centerX
      const ny = (e.clientY - centerY) / centerY

      const clampedX = Math.max(-1, Math.min(1, nx))
      const clampedY = Math.max(-1, Math.min(1, ny))

      onTrack(clampedX, clampedY)
    }

    window.addEventListener('mousemove', handleMouseMove, { passive: true })
    return () => window.removeEventListener('mousemove', handleMouseMove)
  }, [enabled, onTrack])
}
