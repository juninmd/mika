import { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { layerStyle } from '../lib/layer-styles'

export type ContextMenuItem = {
  id: string
  label: string

  hint?: string
  /** Toggle items show their state as a switch; the label names the setting, never the action. */
  checked?: boolean
  icon?: React.ReactNode
  onSelect: () => void

  separatorBefore?: boolean
  destructive?: boolean
  /** Destructive items ask twice in place: the first click arms the item and shows this label. */
  confirmLabel?: string
}

export type ContextMenuAnchor = { x: number; y: number }

type Props = {
  anchor: ContextMenuAnchor | null
  items: ContextMenuItem[]
  onClose: () => void
}

const MENU_WIDTH = 232
const ITEM_HEIGHT = 34
const MENU_PADDING = 8
const VIEWPORT_MARGIN = 8
// The dock owns the bottom strip; a menu reaching it reads as tucked under the dock.
const DOCK_CLEARANCE = 76

function estimateMenuHeight(items: ContextMenuItem[]): number {
  const separatorCount = items.filter((i) => i.separatorBefore).length
  return items.length * ITEM_HEIGHT + separatorCount * 13 + MENU_PADDING * 2
}

function clampToViewport(anchor: ContextMenuAnchor, width: number, height: number): { left: number; top: number } {
  const maxLeft = Math.max(VIEWPORT_MARGIN, window.innerWidth - width - VIEWPORT_MARGIN)
  const maxTop = Math.max(VIEWPORT_MARGIN, window.innerHeight - height - DOCK_CLEARANCE)
  return {
    left: Math.max(VIEWPORT_MARGIN, Math.min(anchor.x, maxLeft)),
    top: Math.max(VIEWPORT_MARGIN, Math.min(anchor.y, maxTop)),
  }
}

export const MikaContextMenu = memo(function MikaContextMenu({ anchor, items, onClose }: Props) {
  const { t } = useI18n()
  const menuRef = useRef<HTMLDivElement>(null)
  const [focusIdx, setFocusIdx] = useState(0)
  const [armedId, setArmedId] = useState<string | null>(null)
  const [coords, setCoords] = useState<{ left: number; top: number } | null>(null)

  useEffect(() => {
    if (anchor) setFocusIdx(0)
    setArmedId(null)
  }, [anchor])

  useLayoutEffect(() => {
    if (!anchor || !menuRef.current) return
    const rect = menuRef.current.getBoundingClientRect()
    setCoords(clampToViewport(anchor, rect.width || MENU_WIDTH, rect.height || estimateMenuHeight(items)))
  }, [anchor, items])

  useEffect(() => {
    if (!anchor) return
    const first = menuRef.current?.querySelector<HTMLButtonElement>('button')
    first?.focus()
  }, [anchor])

  const move = useCallback(
    (delta: number) => {
      const buttons = menuRef.current?.querySelectorAll<HTMLButtonElement>('button')
      if (!buttons?.length) return
      const next = (focusIdx + delta + buttons.length) % buttons.length
      setFocusIdx(next)
      buttons[next]?.focus()
    },
    [focusIdx],
  )

  const focusFirst = useCallback(() => {
    const buttons = menuRef.current?.querySelectorAll<HTMLButtonElement>('button')
    setFocusIdx(0)
    buttons?.[0]?.focus()
  }, [])

  const focusLast = useCallback(() => {
    const buttons = menuRef.current?.querySelectorAll<HTMLButtonElement>('button')
    if (!buttons?.length) return
    const lastIdx = buttons.length - 1
    setFocusIdx(lastIdx)
    buttons[lastIdx]?.focus()
  }, [])

  useEffect(() => {
    if (!anchor) return

    const keyActions: Record<string, () => void> = {
      Escape: onClose,
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Home: focusFirst,
      End: focusLast,
    }

    const onKeyDown = (event: KeyboardEvent) => {
      const action = keyActions[event.key]
      if (!action) return
      event.preventDefault()
      action()
    }

    const onPointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose()
    }

    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('blur', onClose)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('blur', onClose)
    }
  }, [anchor, move, onClose, focusFirst, focusLast])

  if (!anchor) return null

  const pos = coords ?? clampToViewport(anchor, MENU_WIDTH, estimateMenuHeight(items))

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label={t('shell.menu.label')}
      className="electron-no-drag menu-pop fixed max-h-[calc(100vh-16px)] flex flex-col"
      style={{ ...layerStyle('dropdown'), left: pos.left, top: pos.top, width: MENU_WIDTH }}
    >
      <div className="overflow-hidden overflow-y-auto rounded-2xl border border-border-subtle bg-popover py-2 shadow-2xl max-h-[calc(100vh-16px)]">
        {items.map((item, idx) => (
          <div key={item.id} className="menu-item-in" style={{ animationDelay: `${Math.min(idx, 8) * 18}ms` }}>
            {item.separatorBefore && <div className="my-1.5 h-px bg-surface-raise-strong" />}
            <button
              type="button"
              {...(item.checked === undefined
                ? { role: 'menuitem' }
                : { role: 'menuitemcheckbox', 'aria-checked': item.checked })}
              onMouseEnter={() => setFocusIdx(idx)}
              onFocus={() => setFocusIdx(idx)}
              onClick={() => {
                if (item.confirmLabel && armedId !== item.id) {
                  setArmedId(item.id)
                  return
                }
                onClose()
                item.onSelect()
              }}
              className={`flex w-full items-center gap-2.5 px-3 text-left text-[12px] font-medium transition-colors focus-visible:outline-none focus-visible:bg-surface-raise-strong focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus ${
                item.destructive
                  ? 'text-danger hover:bg-danger-soft hover:text-danger'
                  : 'text-text-main hover:bg-surface-raise-strong'
              }`}
              style={{ height: ITEM_HEIGHT }}
            >
              <span
                className="flex h-4 w-4 shrink-0 items-center justify-center opacity-80 text-text-muted"
                aria-hidden="true"
              >
                {item.icon}
              </span>
              <span className="flex-1 truncate">{armedId === item.id ? item.confirmLabel : item.label}</span>
              {item.hint && (
                <span className="shrink-0 text-micro font-bold uppercase tracking-wider text-accent">{item.hint}</span>
              )}
              {item.checked !== undefined && (
                <span
                  aria-hidden="true"
                  className={`relative h-[18px] w-8 shrink-0 rounded-full border transition-colors ${
                    item.checked ? 'border-accent-rim bg-accent' : 'border-border-strong bg-surface-raise-strong'
                  }`}
                >
                  <span
                    className={`absolute top-0.5 size-3 rounded-full bg-text-main transition-[left] ${
                      item.checked ? 'left-4' : 'left-0.5'
                    }`}
                  />
                </span>
              )}
            </button>
          </div>
        ))}
      </div>
    </div>
  )
})
