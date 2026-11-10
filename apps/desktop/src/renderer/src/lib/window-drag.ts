import { setWindowDragging } from './effect-quality'
import { rendererLog } from './renderer-logger'
import { getElectronApi } from './shell-api'

const DRAG_THRESHOLD_PX = 4

// Controles do SDK não usam sempre elemento nativo; arrastar um slider tem de mover o thumb, não a janela.
const INTERACTIVE =
  'button, input, select, textarea, label, a, [role="button"], [role="slider"], [role="switch"], [data-sdk-control]'

let activePointerId: number | null = null
let warnedMissingBridge = false

export interface WindowDragOptions {
  onClick?: (event: PointerEvent) => void
}

// O laço nativo engole o pointerup: o fim do arrasto é o primeiro movimento/ponteiro que volta ao webview.
const DRAG_END_EVENTS = ['pointermove', 'pointerdown', 'pointerenter', 'blur'] as const
let cancelNativeDrag: (() => void) | null = null

function beginNativeDrag(): void {
  cancelNativeDrag?.() // um arrasto só por vez: sem listeners nem timer de arrastos anteriores
  setWindowDragging(true)
  const startedAt = performance.now()
  const stop = () => {
    clearTimeout(safety)
    for (const type of DRAG_END_EVENTS) window.removeEventListener(type, end)
    if (cancelNativeDrag === stop) cancelNativeDrag = null
    setWindowDragging(false)
  }
  const end = () => {
    if (performance.now() - startedAt >= 200) stop()
  }
  const safety = setTimeout(stop, 30_000) // rede de segurança: nunca prender o avatar a 30 fps
  cancelNativeDrag = stop
  for (const type of DRAG_END_EVENTS) window.addEventListener(type, end)
}

export function handleWindowDragPointerDown(startEvent: PointerEvent, options?: WindowDragOptions): void {
  if (startEvent.button !== 0 || activePointerId !== null) return

  const target = startEvent.target as HTMLElement | null
  if (target?.closest?.(INTERACTIVE)) return

  let moveBy: ((deltaX: number, deltaY: number) => void) | undefined
  let startDrag: (() => void) | undefined
  try {
    const windowApi = getElectronApi().shell.window
    moveBy = windowApi?.moveBy
    startDrag = windowApi?.startDrag
  } catch {
    moveBy = undefined
  }
  if (!moveBy) {
    // Uma vez por sessão: sem a ponte, cada clique no avatar repetiria o aviso.
    if (!warnedMissingBridge) {
      warnedMissingBridge = true
      rendererLog.warn('window-drag: electronAPI.shell.window.moveBy unavailable')
    }
    return
  }

  const pointerId = startEvent.pointerId
  activePointerId = pointerId
  let startScreenX = startEvent.screenX
  let startScreenY = startEvent.screenY
  let hasDragged = false
  let released = false

  // A janela é frameless e persegue o cursor: sem captura, soltar o botão fora dela nunca entrega o
  // pointerup, e o arrasto ficaria preso — cursor `grabbing` e click-through desligado até reiniciar.
  target?.setPointerCapture?.(pointerId)

  const release = (upEvent?: PointerEvent) => {
    if (released) return
    released = true
    activePointerId = null
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('pointercancel', onPointerUp)
    window.removeEventListener('blur', onWindowBlur)
    target?.releasePointerCapture?.(pointerId)
    delete document.body.dataset.dragging
    if (!hasDragged && upEvent) options?.onClick?.(upEvent)
  }

  const onPointerMove = (moveEvent: PointerEvent) => {
    if (moveEvent.pointerId !== pointerId) return
    const deltaX = moveEvent.screenX - startScreenX
    const deltaY = moveEvent.screenY - startScreenY

    if (!hasDragged) {
      if (Math.hypot(deltaX, deltaY) < DRAG_THRESHOLD_PX) return
      hasDragged = true
      if (startDrag) {
        // The OS move loop owns the mouse from here on and the webview never sees the pointerup.
        release()
        startDrag()
        beginNativeDrag()
        return
      }
      document.body.dataset.dragging = 'true'
    }

    startScreenX = moveEvent.screenX
    startScreenY = moveEvent.screenY

    moveBy(deltaX, deltaY)
  }

  const onPointerUp = (upEvent: PointerEvent) => {
    if (upEvent.pointerId !== pointerId) return
    release(upEvent)
  }

  const onWindowBlur = () => release()

  window.addEventListener('pointermove', onPointerMove)
  window.addEventListener('pointerup', onPointerUp)
  window.addEventListener('pointercancel', onPointerUp)
  window.addEventListener('blur', onWindowBlur)
}
