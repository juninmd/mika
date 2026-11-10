import type * as PIXI from 'pixi.js'
import type { RefObject } from 'react'
import type { AvatarCanvasProps } from './avatar/avatar-canvas.types'
import {
  Live2DAnimator,
  Live2DEngine,
  type Live2DModelInstance,
  layoutModelForStage,
  type loadLive2DModel,
  resolveModelStem,
} from './live2d'

export const applyResponsiveModelLayout = layoutModelForStage

export interface Live2DStageContext {
  containerRef: RefObject<HTMLDivElement | null>
  appRef: RefObject<PIXI.Application | null>
  modelRef: RefObject<Live2DModelInstance | null>
  engineRef: RefObject<Live2DEngine | null>
  onAvatarInteractRef: RefObject<AvatarCanvasProps['onAvatarInteract']>
  isSpeakingRef: RefObject<boolean>
  syncTickerRef: RefObject<(() => void) | null>
  frozenRef: RefObject<boolean>
  onBatteryRef: RefObject<boolean>
  modelPath: string
  volumeRef: AvatarCanvasProps['volumeRef']
  vowelsRef: AvatarCanvasProps['vowelsRef']
  mouseRef: AvatarCanvasProps['mouseRef']
  setLoadError: (error: string | null) => void
  setIsModelLoading: (loading: boolean) => void
}

export function wireLive2DModel(
  ctx: Live2DStageContext,
  app: PIXI.Application,
  model: Awaited<ReturnType<typeof loadLive2DModel>>,
  width: number,
  height: number,
): void {
  const { modelRef, engineRef, volumeRef, vowelsRef, mouseRef, setLoadError, setIsModelLoading } = ctx
  modelRef.current = model
  setLoadError(null)
  setIsModelLoading(false)
  if (!model) return

  applyResponsiveModelLayout(model, width, height)

  const animator = new Live2DAnimator(model, resolveModelStem(model))
  const engine = new Live2DEngine(model, animator)
  engineRef.current = engine
  engine.setLipSyncSources(volumeRef, vowelsRef)
  animator.start()

  app.ticker.add(() => {
    model.update(app.ticker.deltaMS)
    animator.update(app.ticker.deltaMS / 1000)
    if (mouseRef?.current) {
      animator.setCursorTarget(mouseRef.current.x, mouseRef.current.y)
    }
  })

  model.update(16)
  animator.update(16 / 1000)
  app.renderer.render(app.stage)
}
