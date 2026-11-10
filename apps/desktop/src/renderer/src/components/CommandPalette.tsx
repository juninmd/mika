import { toDisplayError } from '@mika/sdk-ui'
import { DURATION, SPRING } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { Search } from 'lucide-react'
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ToolCatalogItem } from '../../../shared/electron-api-types'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { useI18n } from '../hooks/useI18n'
import { isPaletteShortcut } from '../lib/keyboard-shortcuts'
import { layerStyle } from '../lib/layer-styles'
import { getElectronApi } from '../lib/shell-api'
import { useUIStore } from '../store/useUIStore'
import { CommandPaletteResults } from './CommandPaletteResults'
import { buildEffectItems, buildToolItems, groupPaletteItems } from './command-palette-items'
import { buildStaticItems } from './command-palette-static-items'

export const CommandPalette = memo(function CommandPalette() {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [tools, setTools] = useState<ToolCatalogItem[]>([])
  const [toolError, setToolError] = useState<string | null>(null)
  const [isLoadingTools, setIsLoadingTools] = useState(false)
  const [selectedIdx, setSelectedIdx] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const handleDialogTab = useFocusTrap(dialogRef)
  const eAPI = getElectronApi()
  const addNotification = useUIStore((s) => s.addNotification)
  const toggleOverlay = useUIStore((s) => s.toggleOverlay)
  const pinned = useUIStore((s) => s.pinned)
  const setPinned = useUIStore((s) => s.setPinned)

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (isPaletteShortcut(e)) {
        e.preventDefault()
        setOpen((v) => !v)
      }
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  // Closing (Esc, backdrop, picking an item) hands focus back to what had it before the palette opened.
  useEffect(() => {
    if (!open) return
    const previous = document.activeElement
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [open])

  useEffect(() => {
    if (!open) return
    setQuery('')
    setSelectedIdx(0)
    setToolError(null)
    setIsLoadingTools(true)
    eAPI?.runtime
      ?.getTools()
      .then((r) => setTools(r.tools))
      .catch((err) => setToolError(toDisplayError(err, t('shell.palette.toolsLoadFailed'))))
      .finally(() => setIsLoadingTools(false))
    requestAnimationFrame(() => inputRef.current?.focus())
  }, [open, eAPI, t])

  useEffect(() => {
    if (!open) return
    const container = listRef.current
    if (!container) return
    const selectedElement = container.querySelector('[data-selected="true"]') as HTMLElement
    if (selectedElement && selectedIdx >= 0) {
      selectedElement.scrollIntoView({
        block: 'nearest',
        behavior: 'smooth',
      })
    }
  }, [selectedIdx, open])

  const close = useCallback(() => setOpen(false), [])

  const { grouped, flat } = useMemo(() => {
    const deps = { t, eAPI, close, addNotification }
    const staticItems = buildStaticItems({ ...deps, toggleOverlay, pinned, setPinned })
    const toolItems = buildToolItems(tools, deps)
    const effectItems = buildEffectItems(tools, deps)

    const allItems = [...staticItems, ...effectItems, ...toolItems]

    return groupPaletteItems(allItems, query, t)
  }, [query, tools, eAPI, close, addNotification, toggleOverlay, t, setPinned, pinned])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIdx((i) => Math.min(i + 1, flat.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIdx((i) => Math.max(i - 1, 0))
      } else if (e.key === 'Home') {
        e.preventDefault()
        setSelectedIdx(0)
      } else if (e.key === 'End') {
        e.preventDefault()
        setSelectedIdx(Math.max(0, flat.length - 1))
      } else if (e.key === 'Enter' && flat[selectedIdx]) {
        flat[selectedIdx].onSelect()
      }
    },
    [flat, selectedIdx],
  )

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            className="fixed inset-0 bg-black/40 backdrop-blur-sm"
            style={layerStyle('overlay')}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: DURATION.fast }}
            onClick={close}
            aria-hidden="true"
          />

          <motion.div
            ref={dialogRef}
            role="dialog"
            aria-label={t('shell.palette.label')}
            aria-modal="true"
            className="fixed left-1/2 top-[18%] w-[min(92vw,34rem)] -translate-x-1/2"
            style={{ zIndex: 'calc(var(--layer-overlay) + 1)' }}
            initial={{ opacity: 0, y: -14, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.97 }}
            transition={SPRING.soft}
            onKeyDown={handleDialogTab}
          >
            <div className="overflow-hidden rounded-2xl border border-border-subtle bg-popover shadow-[0_24px_80px_rgba(0,0,0,0.7)]">
              <div className="flex items-center gap-3 border-b border-border-subtle px-4 py-3.5 transition-colors focus-within:border-accent-rim">
                <Search size={14} className="shrink-0 text-text-muted" aria-hidden="true" />
                <input
                  ref={inputRef}
                  type="text"
                  placeholder={t('shell.palette.placeholder')}
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value)
                    setSelectedIdx(0)
                  }}
                  onKeyDown={handleKeyDown}
                  className="flex-1 bg-transparent text-[13px] text-text-main placeholder:text-text-muted outline-none"
                  aria-label={t('shell.palette.searchLabel')}
                />
                <kbd className="rounded border border-border-subtle bg-surface-raise px-1.5 py-0.5 font-mono text-caption text-text-muted">
                  ESC
                </kbd>
              </div>

              <CommandPaletteResults
                listRef={listRef}
                t={t}
                flat={flat}
                grouped={grouped}
                query={query}
                selectedIdx={selectedIdx}
                isLoadingTools={isLoadingTools}
                toolError={toolError}
                onRetry={() => {
                  setToolError(null)
                  eAPI?.runtime
                    ?.getTools()
                    .then((r) => setTools(r.tools))
                    .catch((err) => setToolError(toDisplayError(err, t('shell.palette.toolsLoadFailed'))))
                }}
              />

              <div
                className="border-t border-border-subtle px-4 py-2 text-caption font-medium text-text-muted flex items-center gap-2"
                aria-hidden="true"
              >
                <span>↑↓ {t('shell.settingsPalette.nav')}</span>
                <span>·</span>
                <span>Enter {t('shell.settingsPalette.open')}</span>
                <span>·</span>
                <span>Esc {t('shell.settingsPalette.esc')}</span>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  )
})
