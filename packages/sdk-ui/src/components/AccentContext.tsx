import { createContext, type ReactNode, useContext } from 'react'

const DEFAULT_ACCENT_HUE = 186

const AccentContext = createContext<number>(DEFAULT_ACCENT_HUE)

export function AccentProvider({ hue, children }: { hue: number; children: ReactNode }) {
  return <AccentContext.Provider value={hue}>{children}</AccentContext.Provider>
}

export function useAccent(override?: number): number {
  const inherited = useContext(AccentContext)
  return override ?? inherited
}
