import { useEffect } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

function isInDragRegion(el: Element | null): boolean {
  let node: Element | null = el
  for (let i = 0; i < 6 && node && node !== document.body; i++, node = node.parentElement) {
    const inlineRegion = (node as HTMLElement & { style: { webkitAppRegion?: string } }).style?.webkitAppRegion
    const computedRegion = window.getComputedStyle(node as HTMLElement).getPropertyValue('-webkit-app-region')
    const region = inlineRegion || computedRegion
    if (region === 'no-drag') return false
    if (region === 'drag') return true
  }
  return false
}

export function shouldIgnoreAt(x: number, y: number): boolean {
  const el = document.elementFromPoint(x, y)
  if (!el || el === document.body || el === document.documentElement) return true
  return window.getComputedStyle(el).pointerEvents === 'none' && !isInDragRegion(el)
}

interface PolledCursor {
  insideX: number
  insideY: number
  isInside: boolean
}

// While ignoring, the webview receives no DOM mousemove: the Rust cursor poll is the only way back.
// `null` leaves the current state alone (cursor outside the window or a drag in progress).
export function pollIgnoreDecision(
  pos: PolledCursor,
  innerHeight: number,
  dragging: boolean,
  hitTest: (x: number, y: number) => boolean,
): boolean | null {
  if (!pos.isInside || dragging) return null
  return pos.insideY <= innerHeight - DOCK_REVEAL_PX && hitTest(pos.insideX, pos.insideY)
}

const DOCK_REVEAL_PX = 80

/** Unmount must never leave the window ignoring the mouse: nothing else would turn clicks back on. */
export function releaseClickThrough(windowApi: { setIgnoreMouseEvents: (ignore: boolean) => void }): void {
  try {
    windowApi.setIgnoreMouseEvents(false)
  } catch (err) {
    rendererLog.warn('useClickThrough: could not release click-through on cleanup', err)
  }
}

export function useClickThrough(): void {
  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI.shell.window?.setIgnoreMouseEvents) {
      rendererLog.warn('useClickThrough: setIgnoreMouseEvents unavailable')
      return
    }
    const windowApi = eAPI.shell.window

    let lastIgnore = false
    let healthy = true
    let rafId: number | null = null
    let lastX = 0
    let lastY = 0

    const setIgnore = (ignore: boolean) => {
      if (ignore !== lastIgnore) {
        lastIgnore = ignore
        try {
          windowApi.setIgnoreMouseEvents(ignore)
          document.body.dataset.clickThrough = String(ignore)
        } catch (err) {
          healthy = false
          rendererLog.error('useClickThrough: setIgnoreMouseEvents threw', err)
        }
      }
    }

    const onMouseMove = (e: MouseEvent) => {
      if (!healthy) return
      if (document.body.dataset.dragging === 'true') {
        setIgnore(false)
        return
      }
      lastX = e.clientX
      lastY = e.clientY
      if (rafId !== null) return
      rafId = requestAnimationFrame(() => {
        rafId = null
        if (document.body.dataset.dragging === 'true') {
          setIgnore(false)
          return
        }
        setIgnore(shouldIgnoreAt(lastX, lastY))
      })
    }

    const onIpcMouseMove = (pos: { x: number; y: number; insideX: number; insideY: number; isInside: boolean }) => {
      if (!healthy) return

      const ignore = pollIgnoreDecision(
        pos,
        window.innerHeight,
        document.body.dataset.dragging === 'true',
        shouldIgnoreAt,
      )
      if (ignore !== null) setIgnore(ignore)
    }

    const onBlur = () => {
      if (!healthy || document.body.dataset.dragging === 'true') return
      setIgnore(true)
    }

    const unbindIpcMouseMove = windowApi.onMouseMove?.(onIpcMouseMove)

    document.addEventListener('mousemove', onMouseMove, { passive: true })
    window.addEventListener('blur', onBlur)
    return () => {
      document.removeEventListener('mousemove', onMouseMove)
      window.removeEventListener('blur', onBlur)
      if (rafId !== null) cancelAnimationFrame(rafId)
      unbindIpcMouseMove?.()
      releaseClickThrough(windowApi)
      delete document.body.dataset.clickThrough
    }
  }, [])
}

useClickThrough.displayName = 'useClickThrough'
