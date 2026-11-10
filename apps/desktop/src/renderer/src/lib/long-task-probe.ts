import { rendererLog } from './renderer-logger'

// Renderer long-task probe: on in dev builds, and in release only with VITE_MIKA_PROBE_LONG_TASKS=1
// (a per-minute line nobody reads in normal use is noise). Reports only when something actually stalled.
const LONG_TASK_MS = 100

const REPORT_INTERVAL_MS = 60_000

interface Janela {
  tarefas: number
  maiorMs: number
  somaMs: number
  interacaoMaiorMs: number
}

export function emptyWindow(): Janela {
  return { tarefas: 0, maiorMs: 0, somaMs: 0, interacaoMaiorMs: 0 }
}

/**
 * `durationThreshold` only filters `event` entries; the Long Tasks API reports every task over 50 ms,
 * so without this check a "100 ms" probe counted every 60 ms hiccup as a stall.
 */
export function recordEntry(janela: Janela, entry: { entryType: string; duration: number }): void {
  if (entry.duration < LONG_TASK_MS) return
  if (entry.entryType === 'event') {
    janela.interacaoMaiorMs = Math.max(janela.interacaoMaiorMs, Math.round(entry.duration))
    return
  }
  janela.tarefas++
  janela.somaMs += entry.duration
  janela.maiorMs = Math.max(janela.maiorMs, Math.round(entry.duration))
}

export function startLongTaskProbe(): () => void {
  if (!import.meta.env.DEV && import.meta.env.VITE_MIKA_PROBE_LONG_TASKS !== '1') return () => {}
  if (typeof PerformanceObserver === 'undefined') return () => {}

  let janela = emptyWindow()

  const observer = new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) recordEntry(janela, entry)
  })

  // `event` may not exist in packaged Chromium; observing separately keeps one missing type from killing the other.
  for (const type of ['longtask', 'event']) {
    try {
      observer.observe({ type, buffered: true, durationThreshold: LONG_TASK_MS } as PerformanceObserverInit)
    } catch {
      // Unsupported type in this Chromium version; the other metric still works.
    }
  }

  const timer = setInterval(() => {
    if (janela.tarefas === 0 && janela.interacaoMaiorMs === 0) return
    rendererLog.warn('renderer travou no último minuto', {
      tarefas: janela.tarefas,
      maiorMs: janela.maiorMs,
      totalMs: Math.round(janela.somaMs),
      interacaoMaiorMs: janela.interacaoMaiorMs,
    })
    janela = emptyWindow()
  }, REPORT_INTERVAL_MS)

  return () => {
    clearInterval(timer)
    observer.disconnect()
  }
}
