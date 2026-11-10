import { toErrorMessage } from '@mika/sdk/utils/object'
import React, { useEffect, useRef, useState } from 'react'
import { useAvatarWind } from '../hooks/useAvatarWind'
import { useI18n } from '../hooks/useI18n'
import { useRenderSharpness } from '../hooks/useRenderSharpness'
import { publishAvatarFace } from '../lib/avatar-face-bus'
import { prefersReducedMotion, resolveEffectQuality, subscribeWindowDragging } from '../lib/effect-quality'
import { getPowerSnapshot, subscribePower } from '../lib/power-profile'
import { watchRenderPixelRatio } from '../lib/render-pixel-ratio'
import { rendererLog } from '../lib/renderer-logger'
import { handleWindowDragPointerDown } from '../lib/window-drag'
import type { AvatarCanvasProps } from './avatar/avatar-canvas.types'
import { isHeadHitArea } from './avatar/avatar-hit-area'
import { VrmAvatar } from './avatar/vrm-adapter'
import { installVrmDebugBridge } from './avatar/vrm-debug-bridge'
import { gateFrame, vrmFramePolicy } from './avatar/vrm-frame-policy'
import { parseVrmModelPath } from './avatar/vrm-look'
import { projectVrmFace } from './hud/vrm-face-projection'

const QUALITY_RECHECK_MS = 500
/** The HUD anchors around her face; head sway is ignored by the bus, so twice a second is plenty. */
const FACE_PUBLISH_MS = 500

/** Throttled head projection for the HUD layout; returns the new "last published" timestamp. */
function publishFaceEvery(avatar: VrmAvatar, now: number, lastAt: number): number {
  if (now - lastAt <= FACE_PUBLISH_MS) return lastAt
  publishAvatarFace(projectVrmFace(avatar.modelFrame(), avatar.camera, window.innerWidth, window.innerHeight))
  return now
}

export const VrmCanvas = React.memo(function VrmCanvas({
  expression,
  intensity = 1,
  modelPath,
  action = null,
  volumeRef,
  vowelsRef,
  mouseRef,
  isSpeaking = false,
  onAvatarInteract,
}: AvatarCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const avatarRef = useRef<VrmAvatar | null>(null)
  const prevActionRef = useRef<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const { t } = useI18n()
  const onAvatarInteractRef = useRef(onAvatarInteract)
  onAvatarInteractRef.current = onAvatarInteract
  const isSpeakingRef = useRef(isSpeaking)
  isSpeakingRef.current = isSpeaking
  // E2E-only: driveLipSync forces this while volume > 0, so the synthetic drive reaches the real
  // full-range mouth path instead of only the idle-noise cap. This flips the avatar's whole
  // `speaking` flag (also gates body-rig speech gestures, not just the mouth), so real TTS state
  // resumes control the moment the test releases it (volume back to 0).
  const lipSyncSpeakingOverrideRef = useRef<boolean | null>(null)
  // O gancho de teste congela o idle; sem isto o recheck de qualidade (500 ms) o religa em seguida.
  const idleOverrideRef = useRef<boolean | null>(null)
  const expressionRef = useRef(expression)
  expressionRef.current = expression
  const intensityRef = useRef(intensity)
  intensityRef.current = intensity
  // Read per frame, not a dep of the mount effect: `mouseRef` flips between the ref and `undefined`
  // whenever `voice.mouseTracking` changes, and re-running that effect rebuilds the whole model.
  const mouseRefRef = useRef(mouseRef)
  mouseRefRef.current = mouseRef
  const onBatteryRef = useRef(false)
  const syncQualityRef = useRef<(() => void) | null>(null)

  // Bateria e teto do governador de energia (jogo/ocioso baixam o fps do avatar).
  useEffect(
    () =>
      subscribePower((power) => {
        onBatteryRef.current = power.onBattery
        syncQualityRef.current?.()
      }),
    [],
  )

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    let mounted = true
    let raf = 0
    let last = performance.now()
    let lastRender = last
    let minFrameMs = 0
    const avatar = new VrmAvatar(canvas, window.innerWidth, window.innerHeight)
    avatar.setLipSyncSources(volumeRef, vowelsRef)
    avatarRef.current = avatar

    let paused = false
    let qualityCheckedAt = 0
    let facePublishedAt = 0
    let pauseTimer: ReturnType<typeof setTimeout> | null = null

    const handlePaused = () => {
      raf = 0
      if (!pauseTimer) {
        pauseTimer = setTimeout(() => {
          pauseTimer = null
          syncQuality()
        }, QUALITY_RECHECK_MS)
      }
    }

    const frame = (now: number) => {
      // Re-read quality from the loop itself: WebView2 did not always fire focus/visibilitychange,
      // and a loop cancelled on pause stayed frozen until some window happened to gain focus.
      if (now - qualityCheckedAt > QUALITY_RECHECK_MS) syncQuality()
      if (paused) {
        handlePaused()
        return
      }
      raf = requestAnimationFrame(frame)
      const gate = gateFrame(now, last, minFrameMs)
      if (!gate.render) return
      last = gate.last
      const dt = Math.min(0.1, (now - lastRender) / 1000)
      lastRender = now
      const cursor = mouseRefRef.current?.current
      if (cursor) avatar.setCursorTarget(cursor.x, cursor.y)
      avatar.setSpeaking(lipSyncSpeakingOverrideRef.current ?? isSpeakingRef.current)
      avatar.update(dt)
      avatar.render()
      facePublishedAt = publishFaceEvery(avatar, now, facePublishedAt)
    }

    // Mesma política de qualidade do Live2D: nunca congela com a janela visível; pausa só oculta.
    const syncQuality = () => {
      if (pauseTimer) {
        clearTimeout(pauseTimer)
        pauseTimer = null
      }
      qualityCheckedAt = performance.now()
      const reducedMotion = prefersReducedMotion()
      const quality = resolveEffectQuality({
        hidden: document.hidden,
        focused: document.hasFocus(),
        onBattery: onBatteryRef.current,
        ceiling: getPowerSnapshot().ceiling,
        reducedMotion,
        speaking: isSpeakingRef.current,
      })
      const policy = vrmFramePolicy(quality)
      paused = policy.paused
      minFrameMs = policy.minFrameMs
      // The anime grade pass (outline weight, split tone, saturation) is the model's shading
      // identity, not a heavy effect — gating it off whenever the companion window loses OS focus
      // (its normal resting state, sitting over a game or editor) made the avatar look flat and
      // washed out almost all the time. Only frame rate scales down with quality now.
      avatar.setPostFx(true)
      avatar.setQuality(quality)
      // Reduced motion: falar ainda move a boca, mas poses ociosas espontâneas são movimento não essencial.
      avatar.setIdleEnabled(idleOverrideRef.current ?? (policy.idle && !reducedMotion))
      avatar.setReducedMotion(reducedMotion)
      if (!paused && !raf) raf = requestAnimationFrame(frame)
    }
    syncQualityRef.current = syncQuality
    const unsubscribeDrag = subscribeWindowDragging(syncQuality)

    // Instalada só após o primeiro frame pintado: window.__mikaVrm existindo cedo demais deixava o
    // E2E (Boolean(window.__mikaVrm) como sinal de "pronto") tirar screenshot em cima do canvas
    // ainda em branco — this.vrm começa null, então readArmRotations()/readLife() respondiam com
    // valores triviais (repouso) mesmo sem modelo nenhum carregado (haru-vrm-idle.png).
    let removeDebugBridge: (() => void) | null = null

    const { url, look } = parseVrmModelPath(modelPath)
    avatar
      .load(url, look)
      .then(() => {
        if (!mounted) return
        setLoading(false)
        avatar.setExpression(expressionRef.current, intensityRef.current)
        // Um frame imediato, como o Live2D faz depois de carregar. O laço de RAF fica pausado com a
        // janela oculta (`resolveEffectQuality`), e uma carga que termina nesse estado não desenhava
        // nada: ao reexibir sem um `visibilitychange` confiável, o palco do VRM ficava em branco.
        avatar.update(1 / 60)
        avatar.render()
        syncQuality()
        removeDebugBridge = installVrmDebugBridge({
          playAction: (name) => avatar.playAction(name),
          playIdleState: (name) => avatar.playIdleState(name),
          readArmRotations: () => avatar.readArmRotations(),
          readLife: () => avatar.readLife(),
          setIdleEnabled: (enabled) => {
            idleOverrideRef.current = enabled
            avatar.setIdleEnabled(enabled)
          },
          driveLipSync: (volume, vowels) => {
            lipSyncSpeakingOverrideRef.current = volume > 0 ? true : null
            avatar.driveLipSyncForTest(volume, vowels)
          },
        })
      })
      .catch((err: unknown) => {
        if (!mounted) return
        setLoading(false)
        setLoadError(toErrorMessage(err))
        rendererLog.error('[VrmCanvas] modelo VRM falhou ao carregar', err)
      })

    const handleResize = () => {
      avatar.resize(window.innerWidth, window.innerHeight)
      // Redimensionar limpa o framebuffer; se o laço estiver pausado, sem este frame a janela
      // ficaria em branco até o próximo `visibilitychange`.
      avatar.render()
    }
    const handlePointerDown = (event: PointerEvent) => {
      handleWindowDragPointerDown(event, {
        onClick: (e) => {
          const bounds = canvas.getBoundingClientRect()
          if (!bounds.width || !bounds.height) return
          const xFrac = (e.clientX - bounds.left) / bounds.width
          const yFrac = (e.clientY - bounds.top) / bounds.height
          avatar.setCursorTarget(xFrac * 2 - 1, yFrac * 2 - 1)
          if (isHeadHitArea({ xFrac, yFrac })) avatar.reactToHeadPat()
          else avatar.reactToPoke()
          onAvatarInteractRef.current?.(e.clientX, e.clientY)
        },
      })
    }
    window.addEventListener('resize', handleResize)
    const stopDprWatch = watchRenderPixelRatio(handleResize)
    window.addEventListener('focus', syncQuality)
    window.addEventListener('blur', syncQuality)
    document.addEventListener('visibilitychange', syncQuality)
    canvas.addEventListener('pointerdown', handlePointerDown)

    return () => {
      mounted = false
      if (pauseTimer) clearTimeout(pauseTimer)
      removeDebugBridge?.()
      cancelAnimationFrame(raf)
      unsubscribeDrag()
      window.removeEventListener('resize', handleResize)
      stopDprWatch()
      window.removeEventListener('focus', syncQuality)
      window.removeEventListener('blur', syncQuality)
      document.removeEventListener('visibilitychange', syncQuality)
      canvas.removeEventListener('pointerdown', handlePointerDown)
      avatar.dispose()
      avatarRef.current = null
      publishAvatarFace(null)
    }
  }, [modelPath, volumeRef, vowelsRef])

  useEffect(() => {
    avatarRef.current?.setExpression(expression, intensity)
  }, [expression, intensity])

  // Re-applied once the model is up: each model path mounts a fresh avatar.
  const wind = useAvatarWind()
  useRenderSharpness()
  useEffect(() => {
    if (!loading) avatarRef.current?.setWind(wind)
  }, [wind, loading])

  useEffect(() => {
    if (action !== null && action !== prevActionRef.current) {
      prevActionRef.current = action
      avatarRef.current?.playAction(action)
    }
  }, [action])

  return (
    <>
      <canvas
        ref={canvasRef}
        className={`electron-no-drag live2d-container h-full w-full ${loading ? 'opacity-0' : 'avatar-enter'}`}
      />
      {loading && (
        <div className="pointer-events-none absolute left-1/2 top-11 z-2 -translate-x-1/2 rounded-full border border-info-rim bg-info-soft px-3 py-1.5 text-xs tracking-wider text-info">
          {t('shell.avatar.loadingVRM')}
        </div>
      )}
      {loadError && (
        <div className="pointer-events-none absolute left-1/2 top-11 z-3 max-w-[420px] -translate-x-1/2 rounded-2xl border border-danger-rim bg-danger-soft px-3 py-2 text-xs leading-relaxed text-danger">
          {t('shell.avatar.errorVRM')}
        </div>
      )}
    </>
  )
})
