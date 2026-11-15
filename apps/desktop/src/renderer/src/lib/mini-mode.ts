export type MiniModeAction = 'leave' | 'close-overlays' | null

export interface MiniModeState {
  mini: boolean
  /** The entry into mini mode was already handled (overlays closed). */
  entered: boolean
  /** Something the user has to read or answer and that only fits the full window (approval, legal, crisis). */
  blocking: boolean
  /** Chat, history or the wheel is open. */
  overlayOpen: boolean
}

/**
 * The mini window only fits her head. On the way in, whatever was open closes (so the switch shrinks
 * her instead of bouncing back); once in, summoning anything that needs room expands her again.
 */
export function miniModeAction({ mini, entered, blocking, overlayOpen }: MiniModeState): MiniModeAction {
  if (!mini) return null
  if (blocking) return 'leave'
  if (!entered) return 'close-overlays'
  return overlayOpen ? 'leave' : null
}
