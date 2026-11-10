import { toErrorMessage } from '@mika/sdk/utils/object'
import { useEffect, useRef } from 'react'
import { useAvatarWind } from '../../../hooks/useAvatarWind'
import { prefersReducedMotion } from '../../../lib/effect-quality'
import { rendererLog } from '../../../lib/renderer-logger'
import { VrmAvatar } from '../../avatar/vrm-adapter'
import { gateFrame } from '../../avatar/vrm-frame-policy'
import { parseVrmModelPath } from '../../avatar/vrm-look'
import { mountShowcaseStage } from '../../avatar/vrm-showcase'
import { frameShowcase } from '../../avatar/vrm-showcase-framing'

/** 3/4 view: the angle a character-select screen rests on, never flat front-on. */
const REST_YAW = 0.42
/** Slow turntable sway around the rest angle (radians, and radians per second). */
const SWAY_AMPLITUDE = 0.3
const SWAY_SPEED = 0.35
/** A full drag across the stage turns the model half a revolution. */
const DRAG_TURN = Math.PI
/** Showcase is decoration next to a settings form: 30 fps is plenty and halves the GPU cost. */
const FRAME_MS = 1000 / 30

export type ShowcaseStatus = 'loading' | 'ready' | 'error'

type Props = {
  modelPath: string
  onStatus: (status: ShowcaseStatus) => void
  /** Bumped by the stage's "special move" button: each new value plays the senbonzakura once. */
  specialMove?: number
}

/** The body action the stage's special-move button shows off. */
export const SPECIAL_MOVE = 'senbonzakura'

type Orbit = { yaw: number; dragging: boolean; lastX: number }

function hostSize(host: HTMLElement): [number, number] {
  return [Math.max(1, host.clientWidth), Math.max(1, host.clientHeight)]
}

/** Drag rotates the model; hovering steers her gaze, the same cursor target the companion window uses. */
function bindPointer(canvas: HTMLCanvasElement, avatar: VrmAvatar, orbit: Orbit): () => void {
  const down = (e: PointerEvent) => {
    orbit.dragging = true
    orbit.lastX = e.clientX
    canvas.setPointerCapture(e.pointerId)
  }
  const move = (e: PointerEvent) => {
    const rect = canvas.getBoundingClientRect()
    if (!rect.width) return
    avatar.setCursorTarget(
      ((e.clientX - rect.left) / rect.width) * 2 - 1,
      ((e.clientY - rect.top) / rect.height) * 2 - 1,
    )
    if (!orbit.dragging) return
    orbit.yaw += ((e.clientX - orbit.lastX) / rect.width) * DRAG_TURN
    orbit.lastX = e.clientX
  }
  const up = () => {
    orbit.dragging = false
  }
  canvas.addEventListener('pointerdown', down)
  canvas.addEventListener('pointermove', move)
  canvas.addEventListener('pointerup', up)
  canvas.addEventListener('pointercancel', up)
  return () => {
    canvas.removeEventListener('pointerdown', down)
    canvas.removeEventListener('pointermove', move)
    canvas.removeEventListener('pointerup', up)
    canvas.removeEventListener('pointercancel', up)
  }
}

/** Frees the GL context right away: browsers cap live contexts, and skins are switched often. */
function releaseContext(canvas: HTMLCanvasElement): void {
  const gl = canvas.getContext('webgl2') ?? canvas.getContext('webgl')
  gl?.getExtension('WEBGL_lose_context')?.loseContext()
}

/**
 * A fresh canvas per mount: its GL context is lost on teardown, and a canvas whose context was lost
 * cannot host a new renderer (StrictMode's double mount hit exactly that). No WebGL → null, and the
 * stage falls back to the poster art instead of crashing the settings surface.
 */
function mountAvatar(host: HTMLElement): { canvas: HTMLCanvasElement; avatar: VrmAvatar } | null {
  const canvas = document.createElement('canvas')
  canvas.className = 'block h-full w-full cursor-grab touch-none active:cursor-grabbing'
  host.appendChild(canvas)
  try {
    return { canvas, avatar: new VrmAvatar(canvas, ...hostSize(host)) }
  } catch (error) {
    rendererLog.warn('[VrmShowcase] WebGL indisponível, palco fica no pôster', { error: toErrorMessage(error) })
    canvas.remove()
    return null
  }
}

type Shot = NonNullable<ReturnType<VrmAvatar['modelFrame']>>

/** Settles the rest pose and expression, then measures and dresses the stage around the model. */
function dressStage(avatar: VrmAvatar, host: HTMLElement): { shot: Shot | null; release?: () => void } {
  avatar.setExpression('happy', 0.35)
  for (let i = 0; i < 10; i++) avatar.update(1 / 60)
  const shot = avatar.modelFrame()
  if (!shot) return { shot }
  const { bounds } = shot
  // `text-accent` on the host is never painted: its computed color is how the theme reaches the rim.
  const accent = getComputedStyle(host).color
  return { shot, release: mountShowcaseStage(avatar.scene, bounds.min.y, bounds.max.x - bounds.min.x, accent) }
}

/** 30 fps loop that idles while the stage is scrolled away or the window is hidden. */
function startLoop(tick: (dt: number) => void, isVisible: () => boolean): () => void {
  let raf = 0
  let last = performance.now()
  let prev = last
  const frame = (now: number) => {
    raf = requestAnimationFrame(frame)
    if (!isVisible() || document.hidden) return
    const gate = gateFrame(now, last, FRAME_MS)
    if (!gate.render) return
    last = gate.last
    tick(Math.min(0.1, (now - prev) / 1000))
    prev = now
  }
  raf = requestAnimationFrame(frame)
  return () => cancelAnimationFrame(raf)
}

type Session = { dispose: () => void; play: (action: string) => void; wind: (strength: number) => void }

function openSession(host: HTMLElement, modelPath: string, onStatus: (status: ShowcaseStatus) => void): Session {
  const mounted = mountAvatar(host)
  if (!mounted) {
    onStatus('error')
    return { dispose: () => {}, play: () => {}, wind: () => {} }
  }
  const { canvas, avatar } = mounted
  const orbit: Orbit = { yaw: 0, dragging: false, lastX: 0 }
  const reduced = prefersReducedMotion()
  avatar.setIdleEnabled(!reduced)
  avatar.setShowcase(true)
  avatar.setReducedMotion(reduced)
  let shot: Shot | null = null
  let clock = 0
  let visible = true
  const cleanups: Array<() => void> = []
  const draw = () => {
    if (!shot) return
    const sway = reduced ? 0 : Math.sin(clock * SWAY_SPEED) * SWAY_AMPLITUDE
    frameShowcase(avatar.camera, shot.bounds, 'stage', REST_YAW + orbit.yaw + sway, shot.head)
    avatar.render()
  }
  const tick = (dt: number) => {
    clock += dt
    avatar.update(dt)
    draw()
  }

  let alive = true
  onStatus('loading')
  const { url, look } = parseVrmModelPath(modelPath)
  avatar
    .load(url, look)
    .then(() => {
      if (!alive) return
      const dressed = dressStage(avatar, host)
      shot = dressed.shot
      if (dressed.release) cleanups.push(dressed.release)
      draw()
      onStatus('ready')
      cleanups.push(startLoop(tick, () => visible))
    })
    .catch((error: unknown) => {
      if (!alive) return
      rendererLog.warn('[VrmShowcase] modelo não carregou na vitrine', { modelPath, error: toErrorMessage(error) })
      onStatus('error')
    })

  const intersection = new IntersectionObserver(([entry]) => {
    visible = entry?.isIntersecting ?? true
  })
  intersection.observe(host)
  const resize = new ResizeObserver(() => {
    avatar.resize(...hostSize(host))
    draw()
  })
  resize.observe(host)
  cleanups.push(
    () => intersection.disconnect(),
    () => resize.disconnect(),
    bindPointer(canvas, avatar, orbit),
  )

  return {
    play: (action) => {
      if (shot) avatar.playAction(action)
    },
    wind: (strength) => avatar.setWind(strength),
    dispose: () => {
      alive = false
      for (const cleanup of cleanups.reverse()) cleanup()
      avatar.dispose()
      releaseContext(canvas)
      canvas.remove()
    },
  }
}

// Live 3D stage of one VRM skin. Mounted per model (keyed by path): switching skins disposes the
// whole renderer, scene and GL context, instead of carrying state from the previous model.
export default function VrmShowcase({ modelPath, onStatus, specialMove = 0 }: Props) {
  const hostRef = useRef<HTMLDivElement>(null)
  const sessionRef = useRef<Session | null>(null)
  const onStatusRef = useRef(onStatus)
  onStatusRef.current = onStatus

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const session = openSession(host, modelPath, (status) => onStatusRef.current(status))
    sessionRef.current = session
    return () => {
      sessionRef.current = null
      session.dispose()
    }
  }, [modelPath])

  useEffect(() => {
    if (specialMove > 0) sessionRef.current?.play(SPECIAL_MOVE)
  }, [specialMove])

  // The stage shows the same wind the companion window has: a new model path is a new session.
  const wind = useAvatarWind()
  // biome-ignore lint/correctness/useExhaustiveDependencies: modelPath swaps the session; the wind is re-sent to the new one.
  useEffect(() => {
    sessionRef.current?.wind(wind)
  }, [wind, modelPath])

  // Bottom inset keeps her feet (and the floor glow) above the name plate.
  return <div ref={hostRef} className="absolute inset-x-0 bottom-16 top-2 text-accent" />
}
