import { describe, expect, it } from 'vitest'
import type { AppStatus } from '../app/app-shell.types'
import { SPEECH_STATUS_DOT, SPEECH_STATUS_LABELS } from '../speech-status'

const ALL_STATUSES: AppStatus[] = [
  'idle',
  'listening',
  'thinking',
  'acting',
  'confirming',
  'speaking',
  'offline',
  'error',
]

/** Contrato do balão de fala: cada estado que o dono precisa distinguir tem de dizer o próprio nome e cor. */
describe('SPEECH_STATUS_LABELS', () => {
  it('cobre todos os estados', () => {
    for (const status of ALL_STATUSES) {
      expect(SPEECH_STATUS_LABELS[status], status).toBeTruthy()
    }
  })

  it('listening e offline não se disfarçam de idle', () => {
    expect(SPEECH_STATUS_LABELS.listening).toBe('status.listening')
    expect(SPEECH_STATUS_LABELS.offline).toBe('status.offline')
    expect(SPEECH_STATUS_LABELS.idle).toBe('shell.status.idle')
  })

  it('só o idle fala do estado idle', () => {
    const usingIdleLabel = ALL_STATUSES.filter((s) => SPEECH_STATUS_LABELS[s] === 'shell.status.idle')
    expect(usingIdleLabel).toEqual(['idle'])
  })
})

describe('SPEECH_STATUS_DOT', () => {
  it('não usa a classe fantasma `bg-warning` — o token do tema é `warn`', () => {
    for (const status of ALL_STATUSES) {
      expect(SPEECH_STATUS_DOT[status], status).not.toMatch(/\bbg-warning\b/)
    }
  })

  it('confirming tem cor de fundo real', () => {
    expect(SPEECH_STATUS_DOT.confirming).toMatch(/\bbg-warn\b/)
  })
})
