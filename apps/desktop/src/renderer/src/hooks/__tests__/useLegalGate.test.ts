import { describe, expect, it } from 'vitest'
import { legalGateStatus } from '../useLegalGate'

const ready = { docStatus: 'ready', version: '2026-10', acceptedVersion: '2026-10', ageConfirmed: true } as const

describe('useLegalGate — legalGateStatus', () => {
  it('aceite em dia: nada a mostrar', () => {
    expect(legalGateStatus(ready)).toEqual({ needed: false, updated: false, failed: false })
  })

  it('versão aceita desatualizada pede aceite e avisa que os textos mudaram', () => {
    expect(legalGateStatus({ ...ready, acceptedVersion: '2025-01' })).toEqual({
      needed: true,
      updated: true,
      failed: false,
    })
  })

  it('nunca aceitou: pede aceite sem o aviso de "mudou"', () => {
    expect(legalGateStatus({ ...ready, acceptedVersion: '' })).toEqual({ needed: true, updated: false, failed: false })
  })

  it('aceite sem confirmação de idade reabre o diálogo', () => {
    expect(legalGateStatus({ ...ready, ageConfirmed: false }).needed).toBe(true)
  })

  it('incógnitas (documento carregando, config não lida) não abrem o diálogo', () => {
    expect(legalGateStatus({ ...ready, docStatus: 'loading', version: '' })).toEqual({
      needed: false,
      updated: false,
      failed: false,
    })
    expect(legalGateStatus({ ...ready, acceptedVersion: null, ageConfirmed: null }).needed).toBe(false)
  })

  it('documento que não carrega NÃO libera o app: o gate fica de pé em estado de falha', () => {
    expect(
      legalGateStatus({ docStatus: 'error', version: '', acceptedVersion: '2026-10', ageConfirmed: true }),
    ).toEqual({
      needed: false,
      updated: false,
      failed: true,
    })
  })
})
