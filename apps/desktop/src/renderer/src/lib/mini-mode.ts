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

/** Where the mini window lives (`app.miniModeStyle`, applied by `mini_mode.rs`). */
export type MiniModeStyle = 'head' | 'dock-right' | 'dock-left'

export function parseMiniModeStyle(value: unknown): MiniModeStyle {
  return value === 'dock-right' || value === 'dock-left' ? value : 'head'
}

export function isDockStyle(style: MiniModeStyle): style is 'dock-right' | 'dock-left' {
  return style !== 'head'
}

/** What needs the full window: the edge dock carries its own compact chat, the head square carries nothing. */
export function miniOverlayOpen(state: {
  style: MiniModeStyle
  showInput: boolean
  showHistory: boolean
  wheelOpen: boolean
}): boolean {
  return state.showHistory || state.wheelOpen || (state.showInput && !isDockStyle(state.style))
}

/** Context menu shapes: picking the active one leaves to the full window, the other one switches to it. */
export function menuWindowMode(
  target: 'head' | 'dock',
  mini: boolean,
  style: MiniModeStyle,
): { mini: boolean; style: MiniModeStyle } {
  if (mini && isDockStyle(style) === (target === 'dock')) return { mini: false, style }
  if (target === 'head') return { mini: true, style: 'head' }
  return { mini: true, style: isDockStyle(style) ? style : 'dock-right' }
}
