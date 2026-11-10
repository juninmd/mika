import type { ReactNode } from 'react'

/** One control per tile, in two-column grids: every setting carries the sentence that says what it changes. */
export function ControlTile({ children }: { children: ReactNode }) {
  return <div className="rounded-xl border border-border-subtle bg-surface-raise/40 p-3">{children}</div>
}
