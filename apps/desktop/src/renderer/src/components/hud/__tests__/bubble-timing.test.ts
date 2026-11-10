import { describe, expect, it } from 'vitest'
import { bubbleDwellMs } from '../../../hooks/mika-events-helpers'
import { BUBBLE_MIN_TAIL_MS, bubbleHideDelayMs } from '../bubble-timing'

const long = 'palavra '.repeat(80).trim()

describe('bubbleHideDelayMs', () => {
  it.each(['thinking', 'acting', 'confirming', 'speaking'] as const)(
    'never schedules a fade while she is still %s — the reply cannot vanish mid-sentence',
    (status) => {
      expect(bubbleHideDelayMs({ status, text: long, shownForMs: 60_000, hovered: false })).toBeNull()
    },
  )

  it('a hovered bubble never fades — the user is reading it', () => {
    expect(bubbleHideDelayMs({ status: 'idle', text: 'oi', shownForMs: 0, hovered: true })).toBeNull()
  })

  it('reading time is proportional to length, counted from when the text was shown', () => {
    const short = bubbleHideDelayMs({ status: 'idle', text: 'oi, tudo bem?', shownForMs: 0, hovered: false })
    const longer = bubbleHideDelayMs({ status: 'idle', text: long, shownForMs: 0, hovered: false })
    expect(longer).toBe(bubbleDwellMs(long))
    expect(longer as number).toBeGreaterThan(short as number)
    expect(bubbleHideDelayMs({ status: 'idle', text: long, shownForMs: 5_000, hovered: false })).toBe(
      bubbleDwellMs(long) - 5_000,
    )
  })

  it('after a long speech the reader still gets a minimum tail instead of an instant cut', () => {
    expect(bubbleHideDelayMs({ status: 'idle', text: 'oi', shownForMs: 90_000, hovered: false })).toBe(
      BUBBLE_MIN_TAIL_MS,
    )
  })

  it('listening does not hold the bubble: the user talking over her is not reading', () => {
    expect(bubbleHideDelayMs({ status: 'listening', text: 'oi', shownForMs: 0, hovered: false })).toBe(
      bubbleDwellMs('oi'),
    )
  })
})
