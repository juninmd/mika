import { describe, expect, it } from 'vitest'
import type { AppStatus } from '../../app/app-shell.types'
import { derivePresence, type PresenceInput } from '../presence-state'

const base: PresenceInput = {
  appStatus: 'idle',
  providerDown: false,
  micEnabled: true,
  hearing: false,
  speechHeld: false,
  ttsIsPlaying: false,
  gaming: false,
}
const kind = (patch: Partial<PresenceInput>) => derivePresence({ ...base, ...patch }).kind

describe('derivePresence', () => {
  it('rests as "ready" with the mic open and as "muted" with it closed', () => {
    expect(kind({})).toBe('ready')
    expect(kind({ micEnabled: false })).toBe('muted')
  })

  it('voice past the VAD gate is "hearing"; a turn still running turns it into "held"', () => {
    expect(kind({ hearing: true })).toBe('hearing')
    expect(kind({ hearing: true, speechHeld: true })).toBe('held')
  })

  it('audio still playing counts as speaking even after appStatus went idle (first/last buffer gap)', () => {
    expect(kind({ ttsIsPlaying: true })).toBe('speaking')
    expect(kind({ appStatus: 'speaking' })).toBe('speaking')
  })

  it.each<[AppStatus, string]>([
    ['thinking', 'thinking'],
    ['acting', 'thinking'],
    ['confirming', 'confirming'],
    ['error', 'error'],
    ['offline', 'offline'],
  ])('appStatus %s → %s', (appStatus, expected) => {
    expect(kind({ appStatus })).toBe(expected)
  })

  it('LLM down beats the listening states — she can hear but cannot answer, and must say so', () => {
    expect(kind({ providerDown: true, hearing: true })).toBe('offline')
    expect(kind({ providerDown: true, micEnabled: false })).toBe('offline')
  })

  it('an error or an approval prompt outranks speech and thinking', () => {
    expect(kind({ appStatus: 'error', ttsIsPlaying: true })).toBe('error')
    expect(kind({ appStatus: 'confirming', ttsIsPlaying: true })).toBe('confirming')
  })

  it('meter source follows what is audible: mic while listening, TTS while speaking, none otherwise', () => {
    expect(derivePresence({ ...base, hearing: true }).meter).toBe('mic')
    expect(derivePresence(base).meter).toBe('mic')
    expect(derivePresence({ ...base, ttsIsPlaying: true }).meter).toBe('tts')
    expect(derivePresence({ ...base, appStatus: 'thinking' }).meter).toBe('none')
    expect(derivePresence({ ...base, micEnabled: false }).meter).toBe('none')
  })

  it('only the resting states are inactive — those are what transparent mode shrinks to a dot', () => {
    expect(derivePresence(base).active).toBe(false)
    expect(derivePresence({ ...base, micEnabled: false }).active).toBe(false)
    expect(derivePresence({ ...base, hearing: true }).active).toBe(true)
    expect(derivePresence({ ...base, appStatus: 'thinking' }).active).toBe(true)
  })

  it('gaming is a modifier, not a state: it rides on top of whatever she is doing', () => {
    const p = derivePresence({ ...base, gaming: true, appStatus: 'thinking' })
    expect(p.kind).toBe('thinking')
    expect(p.gaming).toBe(true)
  })

  it('every kind has its own tone token and label key', () => {
    const kinds = [
      derivePresence(base),
      derivePresence({ ...base, micEnabled: false }),
      derivePresence({ ...base, hearing: true }),
      derivePresence({ ...base, hearing: true, speechHeld: true }),
      derivePresence({ ...base, ttsIsPlaying: true }),
      derivePresence({ ...base, appStatus: 'thinking' }),
      derivePresence({ ...base, appStatus: 'confirming' }),
      derivePresence({ ...base, appStatus: 'error' }),
      derivePresence({ ...base, providerDown: true }),
    ]
    expect(new Set(kinds.map((p) => p.kind)).size).toBe(kinds.length)
    for (const p of kinds) {
      expect(p.tone).toMatch(/^var\(--[a-z-]+\)$/)
      expect(p.labelKey).toBe(`shell.presence.${p.kind}`)
    }
  })
})
