import { rendererLog } from '../../lib/renderer-logger'
import { LIVE2D_ACTION_GROUP, type Live2DActionEntry } from './live2d-action-catalog.generated'
import type { AnimatorState } from './live2d-animator-state'
import type { Live2DModelInstance } from './live2d-loader'

export type ActionTable = Readonly<Record<string, Live2DActionEntry>>

export function startAction(
  s: AnimatorState,
  model: Live2DModelInstance,
  actions: ActionTable,
  name: string,
  priority: number,
): Promise<boolean> | null {
  const entry = actions[name]
  if (!entry) return null

  let started: Promise<boolean>
  try {
    started = model.motion(LIVE2D_ACTION_GROUP, entry.index, priority)
  } catch (e) {
    rendererLog.warn('[Live2DAnimator] falha ao tocar ação', { acao: name, indice: entry.index, erro: e })
    return null
  }

  const params = new Set(entry.params)
  s.activeAction = { name, params, remaining: entry.duration + entry.fadeOut }
  s.actionDrivesBody = s.bodyRig?.listDrivenParams().some((id) => params.has(id)) ?? false
  return started
}

export function pickAvailableAction(s: AnimatorState, actions: ActionTable, candidates: string[]): string | null {
  const available = candidates.filter((name) => name in actions)
  if (available.length === 0) return null
  return s.deterministic ? available[0] : available[Math.floor(Math.random() * available.length)]
}

export function updateActiveAction(s: AnimatorState, dt: number): void {
  if (!s.activeAction) return
  s.activeAction.remaining -= dt
  if (s.activeAction.remaining <= 0) {
    s.activeAction = null
    s.actionDrivesBody = false
  }
}
