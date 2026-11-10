import { useCallback, useEffect, useRef, useState } from 'react'
import type { LlmPullProgress, LlmStatus } from '../../../shared/electron-api-system-types'
import { rendererLog } from '../../src/lib/renderer-logger'
import { getElectronApi } from '../../src/lib/shell-api'

/** Live brain status (`llm:getStatus`), kept fresh by the `llm:status` event the backend pushes
 * after each measured turn and each finished download. */
export function useLlmStatus(): { status: LlmStatus | null; loading: boolean; refresh: () => void } {
  const [status, setStatus] = useState<LlmStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const refresh = useCallback(() => {
    setLoading(true)
    getElectronApi()
      .runtime.getLlmStatus()
      .then((next) => next && setStatus(next))
      .catch((error: unknown) => rendererLog.warn('[useLlmStatus] status indisponível', { error: String(error) }))
      .finally(() => setLoading(false))
  }, [])
  useEffect(() => {
    refresh()
    return getElectronApi().runtime.onLlmStatus((next) => setStatus(next))
  }, [refresh])
  return { status, loading, refresh }
}

export type PullState =
  | { phase: 'idle' }
  | { phase: 'running'; model: string; status: string; completed: number; total: number }
  | { phase: 'done'; model: string }
  | { phase: 'error'; model: string; message: string }

export function pullFraction(state: PullState): number | null {
  if (state.phase !== 'running' || state.total <= 0) return null
  return Math.min(1, state.completed / state.total)
}

export function nextPullState(prev: PullState, event: LlmPullProgress): PullState {
  if (event.done) {
    return event.ok
      ? { phase: 'done', model: event.model }
      : { phase: 'error', model: event.model, message: event.error ?? '' }
  }
  const keep = prev.phase === 'running' && prev.model === event.model ? prev : null
  return {
    phase: 'running',
    model: event.model,
    status: event.status ?? keep?.status ?? '',
    completed: event.completed ?? keep?.completed ?? 0,
    total: event.total ?? keep?.total ?? 0,
  }
}

/** One Ollama download at a time (`llm:pullModel`), with progress from `llm:pull-progress`. */
export function useOllamaPull(activeModel: string | null | undefined) {
  const [state, setState] = useState<PullState>({ phase: 'idle' })
  const stateRef = useRef(state)
  stateRef.current = state
  useEffect(
    () => getElectronApi().runtime.onLlmPullProgress((event) => setState((prev) => nextPullState(prev, event))),
    [],
  )
  useEffect(() => {
    if (activeModel && stateRef.current.phase !== 'running') {
      setState({ phase: 'running', model: activeModel, status: '', completed: 0, total: 0 })
    }
  }, [activeModel])
  const start = useCallback((model: string) => {
    const name = model.trim()
    if (!name) return
    setState({ phase: 'running', model: name, status: '', completed: 0, total: 0 })
    getElectronApi()
      .runtime.pullLlmModel(name)
      .then((res) =>
        setState(res.ok ? { phase: 'done', model: name } : { phase: 'error', model: name, message: res.error ?? '' }),
      )
      .catch((error: unknown) => setState({ phase: 'error', model: name, message: String(error) }))
  }, [])
  const cancel = useCallback(() => {
    void getElectronApi().runtime.cancelLlmPull()
  }, [])
  return { state, start, cancel }
}

export type GpuOffload = { kind: 'gpu' | 'partial' | 'cpu'; percent: number }

/** Quanto do modelo carregado o Ollama pôs na VRAM (`/api/ps` `size_vram`/`size`); `null` sem tamanhos. */
export function gpuOffload(loaded: { sizeBytes: number | null; sizeVramBytes: number | null }): GpuOffload | null {
  const { sizeBytes, sizeVramBytes } = loaded
  if (!sizeBytes || sizeVramBytes == null) return null
  const percent = Math.round((Math.min(sizeVramBytes, sizeBytes) / sizeBytes) * 100)
  if (percent >= 100) return { kind: 'gpu', percent: 100 }
  return { kind: percent === 0 ? 'cpu' : 'partial', percent }
}

export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '—'
  const gb = bytes / 1024 ** 3
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${Math.round(bytes / 1024 ** 2)} MB`
}

export function formatMs(ms: number | null | undefined): string {
  if (ms == null) return '—'
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${Math.round(ms)} ms`
}
