import { describe, expect, it } from 'vitest'
import { dockCard, mediaCard } from '../dock-cards'

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

  it('ignores failures, listings and tools that have nothing to show', () => {
    expect(dockCard('reminder', { success: false, error: 'x' })).toBeNull()
    expect(dockCard('reminder', { success: true, data: { reminders: [] } })).toBeNull()
    expect(dockCard('game_knowledge', { success: true, data: { places: [] } })).toBeNull()
    expect(dockCard('get_time', { success: true, data: { time: '12:00' } })).toBeNull()
    expect(dockCard('game_session', 'not json')).toBeNull()
  })
})

describe('mediaCard', () => {
  it('turns a media reaction into a card naming what she is reacting to', () => {
    const payload = { source: 'media-reaction', context: '…', media: { kind: 'live', title: 'gaules' } }
    expect(mediaCard(payload)).toEqual({ kind: 'media', media: 'live', subject: 'gaules', body: '' })
  })

  it('ignores other proactive thoughts and malformed media', () => {
    expect(mediaCard({ source: 'clipboard-image', context: 'gato' })).toBeNull()
    expect(mediaCard({ source: 'media-reaction', media: { kind: 'podcast', title: 'x' } })).toBeNull()
    expect(mediaCard({ source: 'media-reaction', media: { kind: 'video', title: '  ' } })).toBeNull()
    expect(mediaCard(null)).toBeNull()
  })
})
