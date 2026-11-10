import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  currentRenderPixelRatio,
  MAX_RENDER_PIXEL_RATIO,
  MAX_SHARP_PIXEL_RATIO,
  renderPixelRatio,
  renderSharpnessFromConfig,
  setRenderSharpness,
  watchRenderPixelRatio,
} from '../render-pixel-ratio'

type Listener = () => void

/** `matchMedia` de mentira: guarda um ouvinte por consulta, como o navegador faz por `MediaQueryList`. */
function stubWindow(initialDpr: number) {
  const listeners = new Map<string, Set<Listener>>()
  const host = {
    devicePixelRatio: initialDpr,
    matchMedia: (query: string) => {
      const set = listeners.get(query) ?? new Set<Listener>()
      listeners.set(query, set)
      return {
        addEventListener: (_: 'change', fn: Listener) => set.add(fn),
        removeEventListener: (_: 'change', fn: Listener) => set.delete(fn),
      }
    },
  }
  vi.stubGlobal('window', host)

  /** Troca de monitor: só a consulta armada para a densidade ANTERIOR dispara. */
  const moveToMonitor = (dpr: number) => {
    const armed = [...(listeners.get(`(resolution: ${host.devicePixelRatio}dppx)`) ?? [])]
    host.devicePixelRatio = dpr
    for (const fn of armed) fn()
  }
  const armedCount = () => [...listeners.values()].reduce((sum, set) => sum + set.size, 0)
  return { moveToMonitor, armedCount }
}

afterEach(() => {
  setRenderSharpness('auto')
  vi.unstubAllGlobals()
})

describe('renderPixelRatio', () => {
  it('auto usa a densidade do monitor até o teto', () => {
    expect(renderPixelRatio(1, 'auto')).toBe(1)
    expect(renderPixelRatio(1.5, 'auto')).toBe(1.5)
    expect(renderPixelRatio(3, 'auto')).toBe(MAX_RENDER_PIXEL_RATIO)
  })

  it('densidade ausente ou zero vale 1x em vez de zerar o canvas', () => {
    expect(renderPixelRatio(0, 'auto')).toBe(1)
    expect(renderPixelRatio(undefined, 'auto')).toBe(1)
    expect(renderPixelRatio(Number.NaN, 'sharp')).toBe(1.5)
  })

  it('desempenho renderiza 1x mesmo em tela de alta densidade — é o ponto do modo', () => {
    expect(renderPixelRatio(2, 'performance')).toBe(1)
    expect(renderPixelRatio(3, 'performance')).toBe(1)
  })

  it('nítido superamostra acima do monitor, com teto próprio', () => {
    expect(renderPixelRatio(1, 'sharp')).toBe(1.5)
    expect(renderPixelRatio(2, 'sharp')).toBe(MAX_SHARP_PIXEL_RATIO)
    expect(renderPixelRatio(4, 'sharp')).toBe(MAX_SHARP_PIXEL_RATIO)
    expect(renderPixelRatio(1, 'sharp')).toBeGreaterThan(renderPixelRatio(1, 'auto'))
  })
})

describe('renderSharpnessFromConfig', () => {
  it('aceita os modos do schema e cai em auto para qualquer outra coisa', () => {
    expect(renderSharpnessFromConfig('performance')).toBe('performance')
    expect(renderSharpnessFromConfig('sharp')).toBe('sharp')
    for (const junk of [undefined, null, '', 'ultra', 2, {}]) expect(renderSharpnessFromConfig(junk)).toBe('auto')
  })
})

describe('setRenderSharpness', () => {
  it('muda a densidade em uso e avisa quem desenha', () => {
    stubWindow(2)
    const onChange = vi.fn()
    const stop = watchRenderPixelRatio(onChange)

    setRenderSharpness('performance')
    expect(currentRenderPixelRatio()).toBe(1)
    setRenderSharpness('performance')
    expect(onChange).toHaveBeenCalledTimes(1)

    stop()
    setRenderSharpness('sharp')
    expect(onChange).toHaveBeenCalledTimes(1)
  })
})

describe('watchRenderPixelRatio', () => {
  it('avisa em TODA troca de monitor, não só na primeira', () => {
    const { moveToMonitor } = stubWindow(1)
    const seen: number[] = []
    const stop = watchRenderPixelRatio(() => seen.push(window.devicePixelRatio))

    moveToMonitor(2)
    moveToMonitor(1.25)
    moveToMonitor(2)
    stop()

    expect(seen).toEqual([2, 1.25, 2])
  })

  it('para de avisar e não deixa ouvinte armado depois do descarte', () => {
    const { moveToMonitor, armedCount } = stubWindow(1)
    const onChange = vi.fn()
    const stop = watchRenderPixelRatio(onChange)

    moveToMonitor(2)
    stop()
    moveToMonitor(1)

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(armedCount()).toBe(0)
  })

  it('vira no-op sem matchMedia em vez de derrubar a montagem do avatar', () => {
    vi.stubGlobal('window', { devicePixelRatio: 1 })
    expect(() => watchRenderPixelRatio(() => {})()).not.toThrow()
  })
})
