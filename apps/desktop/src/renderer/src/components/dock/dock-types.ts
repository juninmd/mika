export type { AppStatus } from '../app/app-shell.types'

export type DockButtonProps = {
  children: React.ReactNode
  onClick: () => void
  active?: boolean
  pulse?: boolean
  /** Accessible name — the full sentence with the shortcut, e.g. "Chat (Ctrl+Space)". */
  title: string
  ariaExpanded?: boolean
  disabled?: boolean
  onHoldStart?: () => void
  onHoldEnd?: () => void
  /** Pointer/keyboard landed on this button — drives the dock legend. */
  onPeek?: () => void
}
