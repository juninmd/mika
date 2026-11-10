import '../lib/pixi-unsafe-eval'
import * as PIXI from 'pixi.js'
import React, { useEffect, useRef, useState } from 'react'
import { useAvatarWind } from '../hooks/useAvatarWind'
import { useI18n } from '../hooks/useI18n'
import { useRenderSharpness } from '../hooks/useRenderSharpness'
import { subscribePower } from '../lib/power-profile'
import type { AvatarCanvasProps } from './avatar/avatar-canvas.types'

import type { Live2DEngine, Live2DModelInstance } from './live2d'
import { mountLive2DStage } from './live2d-stage'
;(window as unknown as Record<string, unknown>).PIXI = PIXI

export const Live2DCanvas = React.memo(function Live2DCanvas({
  expression,
  intensity = 1.0,
  modelPath,
  action = null,
  volumeRef,
  vowelsRef,
  mouseRef,
  isSpeaking = false,
  onAvatarInteract,
}: AvatarCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const appRef = useRef<PIXI.Application | null>(null)
  const modelRef = useRef<Live2DModelInstance | null>(null)
  const engineRef = useRef<Live2DEngine | null>(null)
  const prevExpressionRef = useRef<string>('neutral')
  const prevActionRef = useRef<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [isModelLoading, setIsModelLoading] = useState(true)
  const { t } = useI18n()

  const onAvatarInteractRef = useRef(onAvatarInteract)
  onAvatarInteractRef.current = onAvatarInteract

  const isSpeakingRef = useRef(isSpeaking)
  const syncTickerRef = useRef<(() => void) | null>(null)

  const frozenRef = useRef(false)

  const onBatteryRef = useRef(false)

  useEffect(() => {
    isSpeakingRef.current = isSpeaking

    syncTickerRef.current?.()
  }, [isSpeaking])

  // Bateria e teto do governador de energia (jogo/ocioso baixam o fps do ticker).
  useEffect(
    () =>
      subscribePower((power) => {
        onBatteryRef.current = power.onBattery
        syncTickerRef.current?.()
      }),
    [],
  )

  // biome-ignore lint/correctness/useExhaustiveDependencies: mouseRef is a stable ref read fresh on every ticker frame (mouseRef.current) — it is never stale, so adding it as a dep would only force needless model reloads.
  useEffect(() => {
    if (!containerRef.current) return
    // A new model path mounts a fresh engine: flip back to "loading" so the effects that re-apply
    // wind/expression to it run again once it is up (it never left `false` on a skin switch).
    setIsModelLoading(true)
    return mountLive2DStage({
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
      volumeRef,
      vowelsRef,
      mouseRef,
      setLoadError,
      setIsModelLoading,
    })
  }, [modelPath, volumeRef, vowelsRef])

  // The engine is rebuilt per model path and starts neutral: hand it the current expression once it is up.
  const latestExpressionRef = useRef({ expression, intensity })
  latestExpressionRef.current = { expression, intensity }
  useEffect(() => {
    if (isModelLoading) return
    const { expression: current, intensity: currentIntensity } = latestExpressionRef.current
    prevExpressionRef.current = current
    engineRef.current?.setExpression(current, currentIntensity)
  }, [isModelLoading])

  useEffect(() => {
    const engine = engineRef.current
    if (!engine) return
    if (expression !== prevExpressionRef.current) {
      prevExpressionRef.current = expression
      engine.setExpression(expression, intensity)
    }
    if (action !== null && action !== prevActionRef.current) {
      prevActionRef.current = action
      engine.playAction(action)
    }
  }, [expression, intensity, action])

  // Re-applied once the model is up: each model path mounts a fresh engine.
  const wind = useAvatarWind()
  useRenderSharpness()
  useEffect(() => {
    if (!isModelLoading) engineRef.current?.setWind(wind)
  }, [wind, isModelLoading])

  useEffect(() => {
    const engine = engineRef.current
    if (!engine) return
    engine.setLipSyncSources(volumeRef)
    engine.setSpeaking(isSpeaking)
  }, [isSpeaking, volumeRef])

  return (
    <>
      <div
        ref={containerRef}
        className={`electron-no-drag live2d-container cursor-pointer ${isModelLoading ? 'opacity-0' : 'avatar-enter'}`}
      />
      {isModelLoading && (
        <div className="pointer-events-none absolute left-1/2 top-11 z-2 -translate-x-1/2 rounded-full border border-info-rim bg-info-soft px-3 py-1.5 text-xs tracking-wider text-info">
          {t('shell.avatar.loadingLive2D')}
        </div>
      )}
      {loadError && (
        <div className="pointer-events-none absolute left-1/2 top-11 z-3 max-w-[420px] -translate-x-1/2 rounded-2xl border border-danger-rim bg-danger-soft px-3 py-2 text-xs leading-relaxed text-danger">
          {t('shell.avatar.errorLive2D')}
        </div>
      )}
    </>
  )
})
