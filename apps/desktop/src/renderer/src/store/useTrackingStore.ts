import { create } from 'zustand'

interface TrackingState {
  mouseTrackingEnabled: boolean

  setMouseTracking: (enabled: boolean) => void
}

export const useTrackingStore = create<TrackingState>((set) => ({
  mouseTrackingEnabled: true,

  setMouseTracking: (enabled) => set({ mouseTrackingEnabled: enabled }),
}))
