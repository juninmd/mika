// Minimal hook runtime standing in for 'react' in node-only unit tests.
// Real hooks require an active renderer (react-dom/react-test-renderer, neither installed here);
// this reimplements just the slot/effect bookkeeping the two target hooks rely on.

type Cleanup = undefined | (() => void)
type EffectSlot = { deps?: unknown[]; cleanup?: () => void }
type CallbackSlot = { fn: unknown; deps?: unknown[] }

function sameDeps(a: unknown[] | undefined, b: unknown[] | undefined): boolean {
  if (!a || !b || a.length !== b.length) return false
  return a.every((v, i) => Object.is(v, b[i]))
}

class HookEngine {
  private states: unknown[] = []
  private refs: { current: unknown }[] = []
  private callbacks: CallbackSlot[] = []
  private ids: string[] = []
  private effects: EffectSlot[] = []
  private idx = { s: 0, r: 0, c: 0, i: 0, e: 0 }
  private layoutQueue: Array<() => void> = []
  private passiveQueue: Array<() => void> = []
  private idCounter = 0
  dirty = false

  reset(): void {
    this.idx = { s: 0, r: 0, c: 0, i: 0, e: 0 }
    this.layoutQueue = []
    this.passiveQueue = []
    this.dirty = false
  }

  useState<T>(init: T | (() => T)): readonly [T, (next: T | ((prev: T) => T)) => void] {
    const i = this.idx.s++
    if (i >= this.states.length) this.states.push(typeof init === 'function' ? (init as () => T)() : init)
    const setState = (next: T | ((prev: T) => T)) => {
      const prev = this.states[i] as T
      const value = typeof next === 'function' ? (next as (p: T) => T)(prev) : next
      if (!Object.is(value, prev)) {
        this.states[i] = value
        this.dirty = true
      }
    }
    return [this.states[i] as T, setState] as const
  }

  useRef<T>(init: T): { current: T } {
    const i = this.idx.r++
    if (i >= this.refs.length) this.refs.push({ current: init })
    return this.refs[i] as { current: T }
  }

  useCallback<T>(fn: T, deps: unknown[]): T {
    const i = this.idx.c++
    const prev = this.callbacks[i]
    if (!prev || !sameDeps(prev.deps, deps)) this.callbacks[i] = { fn, deps }
    return this.callbacks[i].fn as T
  }

  useId(): string {
    const i = this.idx.i++
    if (i >= this.ids.length) this.ids.push(`:hid${this.idCounter++}:`)
    return this.ids[i]
  }

  private queueEffect(fn: () => Cleanup, deps: unknown[] | undefined, queue: Array<() => void>): void {
    const i = this.idx.e++
    if (!this.effects[i]) this.effects[i] = {}
    const slot = this.effects[i]
    const changed = deps === undefined || !sameDeps(slot.deps, deps)
    if (!changed) return
    queue.push(() => {
      slot.cleanup?.()
      const cleanup = fn()
      slot.cleanup = typeof cleanup === 'function' ? cleanup : undefined
      slot.deps = deps
    })
  }

  useEffect(fn: () => Cleanup, deps?: unknown[]): void {
    this.queueEffect(fn, deps, this.passiveQueue)
  }

  useLayoutEffect(fn: () => Cleanup, deps?: unknown[]): void {
    this.queueEffect(fn, deps, this.layoutQueue)
  }

  flush(): void {
    const layout = this.layoutQueue
    const passive = this.passiveQueue
    this.layoutQueue = []
    this.passiveQueue = []
    for (const run of layout) run()
    for (const run of passive) run()
  }

  unmount(): void {
    for (const effect of this.effects) effect.cleanup?.()
  }
}

let currentEngine: HookEngine | null = null

function engine(): HookEngine {
  if (!currentEngine) throw new Error('hook called outside renderHook')
  return currentEngine
}

// Exported to stand in for the real 'react' module via vi.mock('react', () => import('./fake-react')).
export const useState = <T>(init: T | (() => T)) => engine().useState(init)
export const useRef = <T>(init: T) => engine().useRef(init)
export const useCallback = <T>(fn: T, deps: unknown[]) => engine().useCallback(fn, deps)
export const useId = () => engine().useId()
export const useEffect = (fn: () => Cleanup, deps?: unknown[]) => engine().useEffect(fn, deps)
export const useLayoutEffect = (fn: () => Cleanup, deps?: unknown[]) => engine().useLayoutEffect(fn, deps)

type Context<T> = { _value: T }
export const createContext = <T>(defaultValue: T): Context<T> => ({ _value: defaultValue })
export const useContext = <T>(ctx: Context<T>) => ctx._value

export function renderHook<TProps, TResult>(hookFn: (props: TProps) => TResult, initialProps: TProps) {
  const hookEngine = new HookEngine()
  let props = initialProps
  let result!: TResult

  function renderOnce(): void {
    const prev = currentEngine
    currentEngine = hookEngine
    hookEngine.reset()
    try {
      result = hookFn(props)
    } finally {
      currentEngine = prev
    }
    hookEngine.flush()
  }
  renderOnce()

  return {
    get current(): TResult {
      return result
    },
    rerender(nextProps?: TProps): void {
      if (nextProps !== undefined) props = nextProps
      renderOnce()
    },
    act(fn: () => void): void {
      fn()
      if (hookEngine.dirty) renderOnce()
    },
    /** Re-renders if a state update landed outside of `act` (e.g. inside an awaited promise). */
    flush(): void {
      if (hookEngine.dirty) renderOnce()
    },
    unmount(): void {
      hookEngine.unmount()
    },
  }
}
