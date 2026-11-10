import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { toDisplayError } from '@mika/sdk-ui'
import * as PIXI from 'pixi.js'
import {
  EFFECT_QUALITY_MAX_FPS,
  prefersReducedMotion,
  resolveEffectQuality,
  subscribeWindowDragging,
} from '../lib/effect-quality'
import { getPowerSnapshot } from '../lib/power-profile'
import { currentRenderPixelRatio, watchRenderPixelRatio } from '../lib/render-pixel-ratio'
import { rendererLog } from '../lib/renderer-logger'
import { handleWindowDragPointerDown } from '../lib/window-drag'
import { isHeadHitArea } from './avatar/avatar-hit-area'
import { installLive2DDebugBridge, loadLive2DModel, PIXI_FULL_DESTROY, showPlaceholder } from './live2d'
import { applyResponsiveModelLayout, type Live2DStageContext, wireLive2DModel } from './live2d-stage-model'

export type { Live2DStageContext } from './live2d-stage-model'

export function mountLive2DStage(ctx: Live2DStageContext): () => void {
  const {
    containerRef,
    appRef,
    modelRef,
    engineRef,
    onAvatarInteractRef,
    isSpeakingRef,
    syncTickerRef,
    frozenRef,
    onBatteryRef,
    modelPath,
    setLoadError,
    setIsModelLoading,
  } = ctx
  const { innerWidth: width, innerHeight: height } = window
  let isMounted = true

  const app = new PIXI.Application({
    width,
    height,
    backgroundAlpha: 0,
    antialias: true,
    autoDensity: true,
    resolution: currentRenderPixelRatio(),
    powerPreference: 'high-performance',
  })
  app.ticker.maxFPS = 60

  const wireUpLoadedModel = (model: Awaited<ReturnType<typeof loadLive2DModel>>) =>
    wireLive2DModel(ctx, app, model, width, height)

  const loadAndWireModel = async () => {
    try {
      const model = await loadLive2DModel(app, modelPath, () => !isMounted)
      if (!isMounted) return
      wireUpLoadedModel(model)
    } catch (err: unknown) {
      if (!isMounted) return
      const message = toErrorMessage(err)
      setLoadError(message)
      setIsModelLoading(false)
      rendererLog.error('[Live2DCanvas] Model failed to load:', err)
      showPlaceholder(app, message)
    }
  }

  const initApp = async () => {
    try {
      if (!isMounted || !containerRef.current) {
        app.destroy(true, PIXI_FULL_DESTROY)
        return
      }

      containerRef.current.appendChild(app.view as unknown as HTMLCanvasElement)
      appRef.current = app

      await loadAndWireModel()
    } catch (err: unknown) {
      setLoadError(toDisplayError(err, t('shell.avatar.canvasInitFailed')))
      setIsModelLoading(false)
      rendererLog.error('[Live2DCanvas] Failed to initialize PixiJS:', err)
    }
  }

  void initApp()

  let resizeTimer: ReturnType<typeof setTimeout> | null = null
  const handleResize = () => {
    if (resizeTimer) return
    resizeTimer = setTimeout(() => {
      resizeTimer = null
      if (!app.renderer) return
      const w = window.innerWidth
      const h = window.innerHeight
      const targetResolution = currentRenderPixelRatio()
      if (app.renderer.resolution !== targetResolution) {
        app.renderer.resolution = targetResolution
      }
      app.renderer.resize(w, h)
      if (modelRef.current) applyResponsiveModelLayout(modelRef.current, w, h)
    }, 100)
  }

  const handlePointerDown = (event: PointerEvent) => {
    handleWindowDragPointerDown(event, {
      onClick: (e) => {
        const engine = engineRef.current
        if (!engine || !appRef.current) return
        const view = appRef.current.view as HTMLCanvasElement
        const bounds = view.getBoundingClientRect()
        if (!bounds.width || !bounds.height) return
        const xFrac = (e.clientX - bounds.left) / bounds.width
        const yFrac = (e.clientY - bounds.top) / bounds.height
        engine.setCursorTarget(xFrac * 2 - 1, yFrac * 2 - 1)
        if (isHeadHitArea({ xFrac, yFrac })) engine.reactToHeadPat()
        else engine.reactToPoke()
        onAvatarInteractRef.current?.(e.clientX, e.clientY)
      },
    })
  }

  const syncTickerActive = () => {
    if (frozenRef.current) {
      app.ticker.stop()
      return
    }
    const quality = resolveEffectQuality({
      hidden: document.hidden,
      focused: document.hasFocus(),
      onBattery: onBatteryRef.current,
      ceiling: getPowerSnapshot().ceiling,
      reducedMotion: prefersReducedMotion(),
      speaking: isSpeakingRef.current,
    })
    if (quality === 'paused') {
      app.ticker.stop()
      return
    }
    app.ticker.maxFPS = EFFECT_QUALITY_MAX_FPS[quality]
    app.ticker.start()
  }
  syncTickerRef.current = syncTickerActive
  const reducedMotionQuery =
    typeof window.matchMedia === 'function' ? window.matchMedia('(prefers-reduced-motion: reduce)') : null
  reducedMotionQuery?.addEventListener?.('change', syncTickerActive)

  const removeDebugBridge = installLive2DDebugBridge({
    getAnimator: () => engineRef.current?.animator ?? null,
    setTickerRunning: (running) => {
      frozenRef.current = !running
      syncTickerActive()
    },

    renderFrame: () => {
      app.renderer.render(app.stage)
    },
  })

  window.addEventListener('resize', handleResize)
  const stopDprWatch = watchRenderPixelRatio(handleResize)
  window.addEventListener('focus', syncTickerActive)
  const unsubscribeDrag = subscribeWindowDragging(syncTickerActive)
  window.addEventListener('blur', syncTickerActive)
  document.addEventListener('visibilitychange', syncTickerActive)
  app.view.addEventListener('pointerdown', handlePointerDown)
  syncTickerActive()

  return () => {
    isMounted = false
    unsubscribeDrag()
    if (resizeTimer) clearTimeout(resizeTimer)
    window.removeEventListener('resize', handleResize)
    stopDprWatch()
    window.removeEventListener('focus', syncTickerActive)
    window.removeEventListener('blur', syncTickerActive)
    document.removeEventListener('visibilitychange', syncTickerActive)
    reducedMotionQuery?.removeEventListener?.('change', syncTickerActive)
    app.view.removeEventListener('pointerdown', handlePointerDown)
    removeDebugBridge()
    frozenRef.current = false
    syncTickerRef.current = null
    modelRef.current = null
    if (engineRef.current) {
      engineRef.current.dispose()
      engineRef.current = null
    }
    if (app.view) app.destroy(true, PIXI_FULL_DESTROY)
    appRef.current = null
  }
}
