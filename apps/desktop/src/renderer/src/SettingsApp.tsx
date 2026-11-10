import { accentVars, REVEAL } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { Search, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { MikaNavRail, SubTabBar, sectionForTab } from './components/settings/MikaNavRail'
import { SettingsCommandPalette } from './components/settings/SettingsCommandPalette'
import { flushPendingSettingsCommits } from './components/settings/SettingsFieldCard'
import { PanelContent } from './components/settings/SettingsPanelContent'
import { SUB_TAB_PANEL_ID, subTabId } from './components/settings/sub-tab-navigation'
import { useI18n } from './hooks/useI18n'
import { useSettingsController } from './hooks/useSettingsController'
import { useTheme } from './hooks/useTheme'
import { toAccentTriple } from './lib/apply-theme'
import { isPaletteShortcut, modifierKeyLabel } from './lib/keyboard-shortcuts'

function SettingsBackground() {
  return (
    <div className="pointer-events-none absolute inset-0">
      <div
        className="absolute inset-0 transition duration-[1400ms] ease-out"
        style={{
          background: `var(--bg, #070812)`,
        }}
      />
      <div
        className="absolute left-0 bottom-0 h-[260px] w-[260px] transition duration-[1400ms] ease-out"
        style={{
          background: 'radial-gradient(circle, color-mix(in srgb, var(--accent) 6%, transparent) 0%, transparent 70%)',
          filter: 'blur(48px)',
          transform: 'translate(-20%, 20%)',
        }}
      />
    </div>
  )
}

function TopBar({
  status,
  onOpenPalette,
  onClose,
}: {
  status: string
  onOpenPalette: () => void
  onClose: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="relative z-20 flex h-11 items-center justify-between border-b border-border-subtle px-4 shrink-0">
      <div className="flex items-center gap-2 text-caption">
        <span className="text-text-muted tabular-nums">{status}</span>
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onOpenPalette}
          aria-label={t('shell.settingsPalette.triggerLabel')}
          className="flex items-center gap-1.5 rounded-lg border border-border-subtle bg-bg-surface px-2.5 py-1 text-caption text-text-muted transition hover:text-text-main"
        >
          <Search size={12} />
          <span>{t('button.search')}</span>
          <kbd className="ml-1 rounded bg-surface-raise px-1 py-0.5 font-sans text-caption">{modifierKeyLabel()} K</kbd>
        </button>
        <button
          type="button"
          onClick={onClose}
          aria-label={t('button.close')}
          title={`${t('button.close')} (Esc)`}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border-subtle bg-bg-surface text-text-muted transition hover:bg-surface-raise hover:text-text-main"
        >
          <X size={13} />
        </button>
      </div>
    </div>
  )
}

export function SettingsApp() {
  const { currentTheme } = useTheme()
  const ctrl = useSettingsController()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const openPalette = useCallback(() => setPaletteOpen(true), [])
  const closeWindow = useCallback(() => {
    try {
      window.close()
    } catch {
      // no-op
    }
  }, [])

  useEffect(() => {
    const handleBeforeUnload = () => {
      void flushPendingSettingsCommits()
    }
    window.addEventListener('beforeunload', handleBeforeUnload)
    return () => window.removeEventListener('beforeunload', handleBeforeUnload)
  }, [])

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return
      if (isPaletteShortcut(e)) {
        e.preventDefault()
        setPaletteOpen((v) => !v)
        return
      }
      if (e.key === 'Escape' && !paletteOpen) {
        e.preventDefault()
        closeWindow()
      }
    }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [paletteOpen, closeWindow])

  const accentHue = toAccentTriple(currentTheme.colors.accent)?.h ?? 275
  const activeSection = ctrl.activeTabId ? sectionForTab(ctrl.activeTabId) : null

  if (ctrl.loading) return <SettingsSkeleton />

  return (
    <div
      className="relative flex h-screen w-screen overflow-hidden font-sans antialiased text-text-main"
      style={accentVars(accentHue)}
    >
      <SettingsBackground />

      {paletteOpen && (
        <SettingsCommandPalette
          declarations={ctrl.shellDeclarations}
          onSelectTab={(id) => {
            ctrl.setActiveTab(id)
            setPaletteOpen(false)
          }}
          onClose={() => setPaletteOpen(false)}
        />
      )}

      <MikaNavRail
        activeTabId={ctrl.activeTabId}
        declarations={ctrl.declarations}
        onSelectTab={ctrl.setActiveTab}
        health={ctrl.health}
      />

      <main className="relative z-10 flex min-w-0 flex-1 flex-col">
        <TopBar status={ctrl.status} onOpenPalette={openPalette} onClose={closeWindow} />

        <SubTabBar
          activeTabId={ctrl.activeTabId}
          declarations={ctrl.declarations}
          section={activeSection}
          onSelectTab={ctrl.setActiveTab}
        />

        <div className="flex-1 overflow-y-auto [scrollbar-width:thin] [scrollbar-color:var(--border-subtle)_transparent]">
          {/* No exit + `mode="wait"`: the old tab used to fade out for 180 ms before the new one could
              start. The key remounts the page; hero and sections stagger in via REVEAL variants. */}
          <motion.div
            key={ctrl.activeTabId ?? 'empty'}
            role="tabpanel"
            id={SUB_TAB_PANEL_ID}
            aria-labelledby={ctrl.activeTabId ? subTabId(ctrl.activeTabId) : undefined}
            variants={REVEAL.page}
            initial="hidden"
            animate="show"
            className="mx-auto w-full max-w-[1180px] space-y-4 px-5 py-5 pb-12"
          >
            <PanelContent ctrl={ctrl} accentHue={accentHue} />
          </motion.div>
        </div>
      </main>
    </div>
  )
}

function SettingsSkeleton() {
  return (
    <div className="flex h-screen w-screen overflow-hidden" style={{ background: 'var(--bg, #04050e)' }}>
      <div className="w-[136px] border-r border-border-subtle" />
      <div className="flex flex-1 flex-col">
        <div className="h-11 shrink-0 border-b border-border-subtle" />
        <div className="flex shrink-0 gap-2 px-5 py-2">
          <div className="h-6 w-20 rounded-lg bg-surface-raise animate-pulse" />
          <div className="h-6 w-24 rounded-lg bg-surface-raise animate-pulse" />
          <div className="h-6 w-16 rounded-lg bg-surface-raise animate-pulse" />
        </div>
        <div className="flex-1 p-8 space-y-5">
          <div className="h-8 w-64 rounded-lg bg-surface-raise animate-pulse" />
          <div className="h-40 rounded-2xl bg-surface-raise animate-pulse" />
          <div className="grid grid-cols-3 gap-4">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-28 rounded-2xl bg-surface-raise animate-pulse" />
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
