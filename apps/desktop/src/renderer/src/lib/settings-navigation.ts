import { useEffect } from 'react'

/** A feature panel asking Settings to switch tab (e.g. Privacy → "O que a Mika lembra" → Memória). */
const EVENT = 'mika:settings-navigate'

export function navigateSettingsTab(tabId: string): void {
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: tabId }))
}

export function useSettingsNavigation(onNavigate: (tabId: string) => void): void {
  useEffect(() => {
    const listener = (event: Event) => {
      const tabId = (event as CustomEvent<unknown>).detail
      if (typeof tabId === 'string' && tabId) onNavigate(tabId)
    }
    window.addEventListener(EVENT, listener)
    return () => window.removeEventListener(EVENT, listener)
  }, [onNavigate])
}
