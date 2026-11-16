/** How much of the edge dock is on screen (`EdgeDock.tsx`). */
export type DockVisibility = 'open' | 'strip' | 'hidden'

export interface DockVisibilityState {
  /** `app.dockAutoHide`. */
  autoHide: boolean
  /** A fullscreen app is in front and `app.dockDiscreet` is on (`dock_discreet.rs`). */
  discreet: boolean
  chatOpen: boolean
  /** Pointer over the dock, she is speaking/listening/thinking, or a bubble or card is up. */
  active: boolean
  /** Inactive long enough to tuck away. */
  idle: boolean
}

/**
 * An open chat always wins: the user called her (the global talk shortcut does exactly that over a
 * fullscreen game). Discreet mode hides everything else and lets every click through; otherwise,
 * idle with auto-hide on, the dock tucks into a thin strip that the pointer brings back.
 */
export function dockVisibility(s: DockVisibilityState): DockVisibility {
  if (s.chatOpen) return 'open'
  if (s.discreet) return 'hidden'
  return s.autoHide && s.idle && !s.active ? 'strip' : 'open'
}
