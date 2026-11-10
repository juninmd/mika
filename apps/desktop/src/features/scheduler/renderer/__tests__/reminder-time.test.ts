import { describe, expect, it } from 'vitest'
import { formatDue, minutesUntilTomorrowAt9, quickWhenArgs, toHhMm } from '../reminder-time'

const t = (key: string) =>
  ({ 'settings.scheduler.due.today': 'hoje', 'settings.scheduler.due.tomorrow': 'amanhã' })[key] ?? key

describe('reminder quick chips', () => {
  // `atTime: "09:00"` rolls to tomorrow only once 9h has passed; at 08:00 it would fire TODAY.
  it('"amanhã 9h" lands on tomorrow even before 9h today', () => {
    const now = new Date(2026, 8, 28, 8, 0)
    expect(minutesUntilTomorrowAt9(now)).toBe(25 * 60)
    expect(quickWhenArgs('tomorrow9', now)).toEqual({ amount: 25 * 60, unit: 'minutes' })
  })

  it('relative chips map to the tool amount+unit', () => {
    const now = new Date(2026, 8, 28, 21, 54)
    expect(quickWhenArgs('in10m', now)).toEqual({ amount: 10, unit: 'minutes' })
    expect(quickWhenArgs('in1h', now)).toEqual({ amount: 1, unit: 'hours' })
  })
})

describe('reminder due label', () => {
  const now = new Date(2026, 8, 28, 20, 0)

  it('says today/tomorrow with HH:mm and never seconds', () => {
    const today = new Date(2026, 8, 28, 21, 54, 37).getTime()
    expect(formatDue(today, now, 'pt-BR', t)).toBe('hoje 21:54')
    expect(formatDue(new Date(2026, 8, 29, 9, 0).getTime(), now, 'pt-BR', t)).toBe('amanhã 09:00')
  })

  it('edits start from the reminder local HH:mm', () => {
    expect(toHhMm(new Date(2026, 8, 28, 7, 5).getTime())).toBe('07:05')
  })
})
