import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { handleWindowDragPointerDown as HandleWindowDragPointerDown } from '../window-drag'

// `shell-api.ts` memoizes the resolved bridge in a module-level singleton, so each test must load
// `window-drag` fresh (vi.resetModules) after stubbing globals — otherwise a prior test's bridge
// leaks into the next one regardless of what that test's `window` looks like.
async function loadHandler(): Promise<typeof HandleWindowDragPointerDown> {
  const mod = await import('../window-drag')
  return mod.handleWindowDragPointerDown
}

describe('handleWindowDragPointerDown', () => {
  const moveBy = vi.fn()
  const listeners: Record<string, ((e: unknown) => void)[]> = {}

  beforeEach(() => {
    vi.resetModules()
    listeners.pointermove = []
    listeners.pointerup = []
    listeners.pointercancel = []

    const fakeWindow = {
      addEventListener: vi.fn((type: string, cb: (e: unknown) => void) => {
        listeners[type]?.push(cb)
      }),
      removeEventListener: vi.fn((type: string, cb: (e: unknown) => void) => {
        listeners[type] = listeners[type]?.filter((fn) => fn !== cb)
      }),
      electronAPI: {
        shell: {
          window: {
            moveBy,
          },
        },
      },
    }

    const fakeDocument = {
      body: {
        dataset: {} as Record<string, string>,
      },
    }

    vi.stubGlobal('window', fakeWindow)
    vi.stubGlobal('document', fakeDocument)
    vi.stubGlobal('PointerEvent', class PointerEvent {})
    moveBy.mockClear()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  function dispatchMove(screenX: number, screenY: number) {
    for (const fn of listeners.pointermove ?? []) {
      fn({ screenX, screenY } as PointerEvent)
    }
  }

  function dispatchUp(clientX = 0, clientY = 0) {
    for (const fn of listeners.pointerup ?? []) {
      fn({ clientX, clientY } as PointerEvent)
    }
  }

  it('does not start a drag session when the bridge is unavailable, and warns once', async () => {
    vi.stubGlobal('window', {
      addEventListener: vi.fn((type: string, cb: (e: unknown) => void) => {
        listeners[type]?.push(cb)
      }),
      removeEventListener: vi.fn(),
    })
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    const event = { button: 0, screenX: 10, screenY: 10, target: null } as unknown as PointerEvent

    handleWindowDragPointerDown(event, { onClick })

    expect(document.body.dataset.dragging).toBeUndefined()
    expect(listeners.pointermove).toHaveLength(0)
    expect(warnSpy).toHaveBeenCalledTimes(1)
  })

  it('does not start a drag session when moveBy is missing, and warns once', async () => {
    vi.stubGlobal('window', {
      addEventListener: vi.fn((type: string, cb: (e: unknown) => void) => {
        listeners[type]?.push(cb)
      }),
      removeEventListener: vi.fn(),
      electronAPI: { shell: { window: {} } },
    })
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    const event = { button: 0, screenX: 10, screenY: 10, target: null } as unknown as PointerEvent

    handleWindowDragPointerDown(event, { onClick })

    expect(document.body.dataset.dragging).toBeUndefined()
    expect(listeners.pointermove).toHaveLength(0)
    expect(warnSpy).toHaveBeenCalledTimes(1)
  })

  it('hands the drag to the native OS move loop once past the threshold, without a click', async () => {
    const startDrag = vi.fn()
    ;(window as unknown as { electronAPI: { shell: { window: object } } }).electronAPI.shell.window = {
      moveBy,
      startDrag,
    }
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    handleWindowDragPointerDown({ button: 0, screenX: 0, screenY: 0, target: null } as unknown as PointerEvent, {
      onClick,
    })

    dispatchMove(1, 1)
    expect(startDrag).not.toHaveBeenCalled()
    dispatchMove(20, 20)
    dispatchMove(40, 40)
    dispatchUp()

    expect(startDrag).toHaveBeenCalledTimes(1)
    expect(moveBy).not.toHaveBeenCalled()
    expect(onClick).not.toHaveBeenCalled()
    // Só o vigia de fim-de-arrasto fica (o laço nativo engole o pointerup); o handler do arrasto saiu.
    expect(listeners.pointermove).toHaveLength(1)
  })

  it('ignores non-primary buttons (e.g. right click)', async () => {
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    const event = { button: 2, screenX: 0, screenY: 0, target: null } as unknown as PointerEvent
    handleWindowDragPointerDown(event, { onClick })

    dispatchMove(20, 20)
    dispatchUp()

    expect(moveBy).not.toHaveBeenCalled()
    expect(onClick).not.toHaveBeenCalled()
  })

  it('calls onClick when mouse 1 is clicked without dragging past threshold', async () => {
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    const event = { button: 0, screenX: 10, screenY: 10, target: null } as unknown as PointerEvent
    handleWindowDragPointerDown(event, { onClick })

    dispatchMove(11, 11)
    dispatchUp(5, 5)

    expect(moveBy).not.toHaveBeenCalled()
    expect(onClick).toHaveBeenCalledTimes(1)
    expect(document.body.dataset.dragging).toBeUndefined()
  })

  it('drags the window via moveBy and sets dragging dataset when moved past threshold', async () => {
    const handleWindowDragPointerDown = await loadHandler()
    const onClick = vi.fn()
    const event = { button: 0, screenX: 10, screenY: 10, target: null } as unknown as PointerEvent
    handleWindowDragPointerDown(event, { onClick })

    dispatchMove(20, 15)
    expect(moveBy).toHaveBeenCalledWith(10, 5)
    expect(document.body.dataset.dragging).toBe('true')

    dispatchMove(25, 12)
    expect(moveBy).toHaveBeenCalledWith(5, -3)

    dispatchUp()
    expect(onClick).not.toHaveBeenCalled()
    expect(document.body.dataset.dragging).toBeUndefined()
  })
})
