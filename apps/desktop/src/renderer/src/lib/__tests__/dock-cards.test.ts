import { describe, expect, it } from 'vitest'
import { dockCard } from '../dock-cards'

describe('dockCard', () => {
  it('turns a created reminder into a reminder card', () => {
    const result = {
      success: true,
      data: { id: 'r1', subject: 'Tomar água', dueAt: 1_700_000_000_000, recurrence: 'once' },
    }
    expect(dockCard('reminder', result)).toEqual({
      kind: 'reminder',
      subject: 'Tomar água',
      body: '',
      at: 1_700_000_000_000,
    })
  })

  it('turns a game guide into a hint card that keeps the help-ladder step', () => {
    const result = { success: true, data: { game: 'Hollow Knight', topic: 'Hornet', helpLevel: 'tip', guide: 'Dash.' } }
    expect(dockCard('game_knowledge', result)).toEqual({
      kind: 'hint',
      subject: 'Hollow Knight',
      body: 'Dash.',
      level: 'tip',
    })
  })

  it('turns a narrated session into a recap card, also when the result arrives as JSON text', () => {
    const result = JSON.stringify({ success: true, data: { gameName: 'Hades', commentary: 'Morreu no Asterius.' } })
    expect(dockCard('game_session', result)).toEqual({ kind: 'recap', subject: 'Hades', body: 'Morreu no Asterius.' })
  })

  it('turns the day summary into a daily card with the stats, and only the stats in passenger mode', () => {
    const data = {
      day: '2026-11-16',
      totalMs: 7_200_000,
      totalDuration: '2h',
      sessionCount: 2,
      byGame: [
        { game: 'Hades', duration: '1h' },
        { game: 'Celeste', duration: '1h' },
      ],
      achievements: [{ game: 'Hades', description: 'Fugiu do Tártaro' }],
      deaths: 3,
      currentStreakDays: 4,
      passengerMode: false,
      highlights: [{ game: 'Hades', summary: 'Run tensa contra a Megera.' }],
    }
    expect(dockCard('game_session', { success: true, data })).toEqual({
      kind: 'daily',
      subject: '2h · Hades, Celeste',
      body: 'Run tensa contra a Megera.',
      daily: { sessions: 2, deaths: 3, streak: 4, achievements: ['Fugiu do Tártaro'], passenger: false },
    })
    const { highlights: _memories, ...passengerData } = data
    expect(dockCard('game_session', { success: true, data: { ...passengerData, passengerMode: true } })).toMatchObject({
      body: '',
      daily: { passenger: true, achievements: ['Fugiu do Tártaro'] },
    })
  })

  it('ignores failures, listings and tools that have nothing to show', () => {
    expect(dockCard('reminder', { success: false, error: 'x' })).toBeNull()
    expect(dockCard('reminder', { success: true, data: { reminders: [] } })).toBeNull()
    expect(dockCard('game_knowledge', { success: true, data: { places: [] } })).toBeNull()
    expect(dockCard('get_time', { success: true, data: { time: '12:00' } })).toBeNull()
    expect(dockCard('game_session', 'not json')).toBeNull()
  })
})
