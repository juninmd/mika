import { DURATION, EASE } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { useI18n } from '../../hooks/useI18n'
import { builtInTabLabel, type NavSection } from './MikaNavRail'
import { nextTabIndex, rovingTabIndex, SUB_TAB_PANEL_ID, subTabId } from './sub-tab-navigation'

interface Props {
  activeTabId: string | null
  declarations: Array<{ tabId: string; tabLabel: string }>
  section: NavSection | null
  onSelectTab: (tabId: string) => void
}

// No per-section special-casing — that caused `sistema` and `voz` to know tabs the others couldn't see.
export function SubTabBar({ activeTabId, declarations, section, onSelectTab }: Props) {
  const { t } = useI18n()
  if (!section) return null

  const tabs = section.tabIds.flatMap((tabId) => {
    const tabLabel = declarations.find((d) => d.tabId === tabId)?.tabLabel ?? builtInTabLabel(tabId)
    return tabLabel ? [{ tabId, tabLabel }] : []
  })
  if (tabs.length === 0) return null

  const tabIds = tabs.map((tab) => tab.tabId)
  const handleKeyNavigation = (e: React.KeyboardEvent<HTMLElement>) => {
    const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'))
    const target = nextTabIndex(e.key, buttons.indexOf(document.activeElement as HTMLButtonElement), buttons.length)
    if (target === null) return
    e.preventDefault()
    buttons[target]?.focus()
    buttons[target]?.click()
  }

  return (
    <motion.div
      key={section.id}
      role="tablist"
      aria-orientation="horizontal"
      aria-label={t(section.labelKey)}
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: DURATION.fast, ease: EASE.out }}
      onKeyDown={handleKeyNavigation}
      className="relative z-10 flex shrink-0 items-center gap-1 overflow-x-auto border-b border-border-subtle px-5 py-2 [scrollbar-width:none]"
    >
      {tabs.map((tab) => {
        const isActive = activeTabId === tab.tabId
        return (
          <button
            key={tab.tabId}
            type="button"
            role="tab"
            id={subTabId(tab.tabId)}
            aria-selected={isActive}
            aria-controls={SUB_TAB_PANEL_ID}
            tabIndex={rovingTabIndex(tab.tabId, activeTabId, tabIds)}
            onClick={() => onSelectTab(tab.tabId)}
            className={`whitespace-nowrap rounded-lg border px-3 py-1.5 text-caption transition active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
              isActive
                ? 'border-accent-rim bg-accent-soft font-semibold text-accent'
                : 'border-transparent font-medium text-text-muted hover:border-border-subtle hover:bg-surface-raise/50 hover:text-text-main'
            }`}
          >
            {tab.tabLabel}
          </button>
        )
      })}
    </motion.div>
  )
}
