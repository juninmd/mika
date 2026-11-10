import { memo, useEffect, useRef } from 'react'
import { useEffectQuality } from '../../hooks/useEffectQuality'
import { EFFECT_QUALITY_MAX_FPS } from '../../lib/effect-quality'
import { layerStyle } from '../../lib/layer-styles'
import { currentRenderPixelRatio, watchRenderPixelRatio } from '../../lib/render-pixel-ratio'
import { useEffectsStore } from '../../store/useEffectsStore'
import { createScene } from './scenes'

export const EffectOverlay = memo(function EffectOverlay() {
  const effect = useEffectsStore((s) => s.activeEffect)
  const clearEffect = useEffectsStore((s) => s.clearEffect)
  const quality = useEffectQuality()
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  useEffect(() => {
    if (!effect || quality === 'paused') return
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const scene = createScene(effect, quality)
    const maxFps = EFFECT_QUALITY_MAX_FPS[quality]
    const minFrameMs = 1000 / maxFps
    let raf = 0
    let last = performance.now()

    const resize = () => {
      const dpr = currentRenderPixelRatio()
      canvas.width = Math.round(window.innerWidth * dpr)
      canvas.height = Math.round(window.innerHeight * dpr)
      canvas.style.width = `${window.innerWidth}px`
      canvas.style.height = `${window.innerHeight}px`
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    }
    resize()
    window.addEventListener('resize', resize)
    const stopDprWatch = watchRenderPixelRatio(resize)

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame)
      const dt = now - last
      if (dt < minFrameMs - 1) return
      last = now
      scene.step(ctx, dt, window.innerWidth, window.innerHeight)
      if (scene.done) clearEffect()
    }
    raf = requestAnimationFrame(frame)

    return () => {
      cancelAnimationFrame(raf)
      window.removeEventListener('resize', resize)
      stopDprWatch()

      ctx.clearRect(0, 0, canvas.width, canvas.height)
    }
  }, [effect, quality, clearEffect])

  if (!effect || quality === 'paused') return null

  return <canvas ref={canvasRef} className="pointer-events-none fixed inset-0" style={layerStyle('effects')} />
})
