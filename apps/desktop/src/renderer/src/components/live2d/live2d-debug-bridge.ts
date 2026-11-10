import { rendererLog } from '../../lib/renderer-logger'
import type { Live2DAnimator } from './live2d-animator'

export interface Live2DDebugBridge {
  listActions(): string[]

  playAction(name: string): boolean

  freeze(name: string | null, seconds: number): Promise<boolean>

  probe(overrides: Record<string, number> | null): void

  readParams(ids: string[]): Record<string, number>

  resume(): void
}

declare global {
  interface Window {
    __mikaLive2D?: Live2DDebugBridge
  }
}

interface BridgeHost {
  getAnimator: () => Live2DAnimator | null

  setTickerRunning: (running: boolean) => void

  renderFrame: () => void
}

export function installLive2DDebugBridge(host: BridgeHost): () => void {
  const bridge: Live2DDebugBridge = {
    listActions: () => host.getAnimator()?.listActions() ?? [],

    playAction: (name) => host.getAnimator()?.playAction(name) ?? false,

    freeze: async (name, seconds) => {
      const animator = host.getAnimator()
      if (!animator) return false
      host.setTickerRunning(false)
      const played = await animator.freeze(seconds, name ?? undefined)

      host.renderFrame()
      if (!played) {
        rendererLog.warn('[Live2D] freeze pediu uma ação que esta skin não tem', {
          acao: name,
          disponiveis: animator.listActions(),
        })
      }
      return played
    },

    readParams: (ids) => host.getAnimator()?.readParams(ids) ?? {},

    probe: (overrides) => {
      host.getAnimator()?.setProbeOverride(overrides)
    },

    resume: () => {
      host.getAnimator()?.setProbeOverride(null)
      host.getAnimator()?.unfreeze()
      host.setTickerRunning(true)
    },
  }

  window.__mikaLive2D = bridge
  return () => {
    if (window.__mikaLive2D === bridge) window.__mikaLive2D = undefined
  }
}
