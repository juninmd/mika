import { useEffect, useMemo, useState } from 'react'
import { computeHudLayout, type HudLayout } from '../components/hud/hud-layout'
import { getAvatarFace, subscribeAvatarFace } from '../lib/avatar-face-bus'

function readViewport() {
  return { width: window.innerWidth, height: window.innerHeight }
}

/** HUD anchors for the window size and where her face is; recomputed at most once per frame while resizing. */
export function useHudLayout(): HudLayout {
  const [viewport, setViewport] = useState(readViewport)
  const [face, setFace] = useState(getAvatarFace)

  useEffect(() => subscribeAvatarFace(setFace), [])

  useEffect(() => {
    let frame: number | null = null
    const onResize = () => {
      if (frame !== null) return
      frame = requestAnimationFrame(() => {
        frame = null
        setViewport((prev) => {
          const next = readViewport()
          return prev.width === next.width && prev.height === next.height ? prev : next
        })
      })
    }
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('resize', onResize)
      if (frame !== null) cancelAnimationFrame(frame)
    }
  }, [])

  return useMemo(() => computeHudLayout(viewport, face), [viewport, face])
}
