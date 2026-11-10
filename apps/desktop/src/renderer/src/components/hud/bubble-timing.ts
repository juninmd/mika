import { bubbleDwellMs } from '../../hooks/mika-events-helpers'
import type { AppStatus } from '../app/app-shell.types'

/** Floor after a long speech: the text already sat there while she spoke, but a cut to nothing reads as a glitch. */
export const BUBBLE_MIN_TAIL_MS = 2_500

const BUSY: ReadonlySet<AppStatus> = new Set(['thinking', 'acting', 'confirming', 'speaking'])

/**
 * When the speech bubble should fade, counted from now — or `null` to keep it.
 *
 * Reading time is proportional to length (`bubbleDwellMs`) and counts from when the text was first
 * shown; while she is still thinking or speaking, or while the pointer rests on the bubble, it
 * never fades.
 */
export function bubbleHideDelayMs(input: {
  status: AppStatus
  text: string
  shownForMs: number
  hovered: boolean
}): number | null {
  if (input.hovered || BUSY.has(input.status)) return null
  return Math.max(BUBBLE_MIN_TAIL_MS, bubbleDwellMs(input.text) - Math.max(0, input.shownForMs))
}
