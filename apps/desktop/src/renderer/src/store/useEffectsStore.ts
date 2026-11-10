import { create } from 'zustand'
import type { MatchEffectCommand, MatchEffectId } from '../lib/match-effect'

interface EffectsState {
  activeEffect: MatchEffectId | null

  startedAt: number

  applyMatchEffect: (command: MatchEffectCommand) => void
  clearEffect: () => void
}

export const useEffectsStore = create<EffectsState>((set, get) => ({
  activeEffect: null,
  startedAt: 0,

  applyMatchEffect: (command) => {
    const { activeEffect } = get()

    if (command.mode === 'toggle' && activeEffect === command.effect) {
      set({ activeEffect: null, startedAt: 0 })
      return
    }
    set({ activeEffect: command.effect, startedAt: Date.now() })
  },

  clearEffect: () => set({ activeEffect: null, startedAt: 0 }),
}))
