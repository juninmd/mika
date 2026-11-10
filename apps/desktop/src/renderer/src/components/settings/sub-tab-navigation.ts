/** Id of the tab button; the panel points back at it with `aria-labelledby`. */
export const subTabId = (tabId: string): string => `settings-tab-${tabId}`
export const SUB_TAB_PANEL_ID = 'settings-tabpanel'

/** Index the focus should move to for a tablist key, or `null` when the key is not ours. */
export function nextTabIndex(key: string, current: number, count: number): number | null {
  if (count === 0) return null
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  if (key === 'ArrowRight') return current < 0 ? 0 : (current + 1) % count
  if (key === 'ArrowLeft') return current < 0 ? 0 : (current - 1 + count) % count
  return null
}

/** Roving tabindex: exactly one tab sits in the Tab order — the active one, else the first. */
export function rovingTabIndex(tabId: string, activeTabId: string | null, tabIds: readonly string[]): 0 | -1 {
  const focusable = activeTabId !== null && tabIds.includes(activeTabId) ? activeTabId : tabIds[0]
  return tabId === focusable ? 0 : -1
}
