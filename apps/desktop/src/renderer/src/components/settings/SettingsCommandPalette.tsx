import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useFocusTrap } from '../../hooks/useFocusTrap'
import { useI18n } from '../../hooks/useI18n'
import { SearchIcon } from '../Icons'
import { buildItems, type PaletteItem, rank } from './settings-palette-items'

type Props = {
  declarations: FeatureSettingsDeclaration[]
  onSelectTab: (id: string) => void
  onClose: () => void
}

export const SettingsCommandPalette = memo(function SettingsCommandPalette({
  declarations,
  onSelectTab,
  onClose,
}: Props) {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
  const [activeIdx, setActiveIdx] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const handleDialogTab = useFocusTrap(dialogRef)

  const defaultGroup = t('shell.settingsPalette.modules')
  const items = useMemo(
    () => buildItems(declarations, onSelectTab, defaultGroup),
    [declarations, onSelectTab, defaultGroup],
  )

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return items.slice(0, 20)
    return items
      .filter(
        (item) =>
          item.label.toLowerCase().includes(q) ||
          item.description?.toLowerCase().includes(q) ||
          item.group.toLowerCase().includes(q) ||
          item.id.toLowerCase().includes(q),
      )
      .sort((a, b) => rank(a, q) - rank(b, q))
      .slice(0, 20)
  }, [items, query])

  // biome-ignore lint/correctness/useExhaustiveDependencies: filtered is a trigger-only dep — it resets the selection whenever the filtered result set changes, even though the body doesn't read filtered itself.
  useEffect(() => {
    setActiveIdx(0)
  }, [filtered])

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  const select = useCallback(
    (item: PaletteItem) => {
      item.action()
      onClose()
    },
    [onClose],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setActiveIdx((i) => Math.min(i + 1, filtered.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setActiveIdx((i) => Math.max(i - 1, 0))
      } else if (e.key === 'Home') {
        e.preventDefault()
        setActiveIdx(0)
      } else if (e.key === 'End') {
        e.preventDefault()
        setActiveIdx(Math.max(0, filtered.length - 1))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        if (filtered[activeIdx]) select(filtered[activeIdx])
      } else if (e.key === 'Escape') {
        onClose()
      }
    },
    [filtered, activeIdx, select, onClose],
  )

  useEffect(() => {
    const item = listRef.current?.children[activeIdx] as HTMLElement | undefined
    item?.scrollIntoView({ block: 'nearest' })
  }, [activeIdx])

  return (
    <div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-label={t('shell.settingsPalette.label')}
      className="fixed inset-0 z-50 flex items-start justify-center pt-[12vh]"
      onKeyDown={handleDialogTab}
    >
      <button
        type="button"
        className="absolute inset-0 cursor-default bg-surface-raise/40"
        aria-label={t('shell.settingsPalette.close')}
        tabIndex={-1}
        onClick={onClose}
      />

      <div className="relative w-full max-w-xl overflow-hidden rounded-2xl border border-border-subtle bg-bg-deep/95 shadow-2xl backdrop-blur-2xl">
        <div className="flex items-center gap-3 border-b border-border-subtle px-4 py-3 transition-colors focus-within:border-accent-rim">
          <SearchIcon className="size-4 shrink-0 text-text-muted" aria-hidden="true" />
          <input
            ref={inputRef}
            className="flex-1 bg-transparent text-sm text-text-main outline-none placeholder:text-text-muted/60"
            placeholder={t('shell.settingsPalette.placeholder')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            aria-label={t('shell.settingsPalette.searchLabel')}
            aria-controls="settings-palette-list"
            aria-activedescendant={filtered[activeIdx] ? `palette-opt-${filtered[activeIdx].id}` : undefined}
          />
          <kbd className="hidden rounded border border-border-subtle bg-surface-raise px-1.5 py-0.5 text-caption font-mono text-text-muted font-medium sm:block">
            ESC
          </kbd>
        </div>

        <div
          ref={listRef}
          id="settings-palette-list"
          role="listbox"
          aria-label={t('shell.settingsPalette.results')}
          className="max-h-80 overflow-y-auto py-1.5 [scrollbar-width:thin]"
        >
          {filtered.length === 0 && (
            <p className="px-4 py-6 text-center text-xs text-text-muted font-medium">
              {t('shell.settingsPalette.noResults')}
            </p>
          )}
          {filtered.map((item, idx) => (
            <button
              key={item.id}
              id={`palette-opt-${item.id}`}
              type="button"
              role="option"
              aria-selected={idx === activeIdx}
              onMouseEnter={() => setActiveIdx(idx)}
              onClick={() => select(item)}
              className={[
                'flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors focus-visible:outline-none',
                idx === activeIdx
                  ? 'bg-surface-raise-strong ring-2 ring-inset ring-focus/70'
                  : 'hover:bg-surface-raise hover:text-text-main',
              ].join(' ')}
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-text-main">{item.label}</p>
                {item.description && <p className="truncate text-caption text-text-muted">{item.description}</p>}
              </div>
              <span className="shrink-0 rounded-md border border-border-subtle bg-surface-raise px-2 py-0.5 text-caption font-medium text-text-muted">
                {item.group}
              </span>
            </button>
          ))}
        </div>

        <div
          className="flex items-center gap-4 border-t border-border-subtle px-4 py-2 text-caption font-medium text-text-muted"
          aria-hidden="true"
        >
          <span>
            <kbd className="font-mono">↑↓</kbd> {t('shell.settingsPalette.nav')}
          </span>
          <span>
            <kbd className="font-mono">↵</kbd> {t('shell.settingsPalette.open')}
          </span>
          <span>
            <kbd className="font-mono">Esc</kbd> {t('shell.settingsPalette.esc')}
          </span>
        </div>
      </div>
    </div>
  )
})
