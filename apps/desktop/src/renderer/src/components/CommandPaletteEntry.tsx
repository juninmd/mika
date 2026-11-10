import type { PaletteItem } from './command-palette-items'

export function GroupHeader({ label, note }: { label: string; note?: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3 px-3 pb-1 pt-2 first:pt-0">
      <span className="text-micro font-black uppercase tracking-[0.2em] text-text-muted">{label}</span>
      {note && <span className="min-w-0 truncate text-caption font-medium text-text-subtle">{note}</span>}
    </div>
  )
}

export function PaletteEntry({
  item,
  selected,
  showGroup,
  hideDescription,
  onSelect,
}: {
  item: PaletteItem
  selected: boolean
  /** The group header already carries this description. */
  hideDescription?: boolean
  /** Only when results are flat: under a group header the badge repeats the header. */
  showGroup: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      data-selected={selected ? 'true' : 'false'}
      className={`relative flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
        selected ? 'bg-surface-raise-strong text-text-main' : 'text-text-main hover:bg-surface-raise'
      }`}
    >
      {selected && (
        <span
          className="absolute left-0 top-2 bottom-2 w-1 rounded-r bg-gradient-to-b from-info to-accent"
          aria-hidden="true"
        />
      )}
      <item.icon aria-hidden="true" size={16} className="shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[12px] font-bold tracking-wide" title={item.label}>
          {item.label}
        </div>
        {item.description && !hideDescription && (
          <div className="truncate text-caption text-text-muted font-medium mt-0.5" title={item.description}>
            {item.description}
          </div>
        )}
      </div>
      {showGroup && (
        <span className="shrink-0 rounded-full border border-border-subtle bg-surface-raise px-2 py-0.5 text-micro font-semibold uppercase tracking-wider text-text-muted">
          {item.group}
        </span>
      )}
      {selected && (
        <kbd className="shrink-0 rounded border border-border-subtle bg-surface-raise px-1.5 py-0.5 font-mono text-caption text-text-muted">
          ↵
        </kbd>
      )}
    </button>
  )
}
