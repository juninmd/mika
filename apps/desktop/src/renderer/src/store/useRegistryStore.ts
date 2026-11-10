import type React from 'react'
import { create } from 'zustand'

type RegistryComponent = React.ComponentType<unknown>

interface RegistryState {
  slots: Record<string, RegistryComponent[]>
  registerComponent: (slotName: string, component: RegistryComponent) => void
  unregisterComponent: (slotName: string, component: RegistryComponent) => void
}

export const useRegistryStore = create<RegistryState>((set) => ({
  slots: {},
  registerComponent: (slotName, component) =>
    set((state) => {
      const existing = state.slots[slotName] || []
      if (existing.includes(component)) return state
      return {
        slots: {
          ...state.slots,
          [slotName]: [...existing, component],
        },
      }
    }),
  unregisterComponent: (slotName, component) =>
    set((state) => {
      const existing = state.slots[slotName] || []
      return {
        slots: {
          ...state.slots,
          [slotName]: existing.filter((c) => c !== component),
        },
      }
    }),
}))
