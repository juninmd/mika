import { describe, expect, it } from 'vitest'
import { statusLabelFor } from '../UpdatePanel'

const idle = {
  installing: false,
  downloaded: false,
  downloading: false,
  checking: false,
  available: false,
  progress: 0,
  checked: true,
  failed: false,
}

describe('update status label', () => {
  it('never claims up to date when the check failed', () => {
    expect(statusLabelFor({ ...idle, failed: true })).toBe('Verificação indisponível')
  })

  it('reports up to date only after a clean check', () => {
    expect(statusLabelFor(idle)).toBe('Atualizado')
    expect(statusLabelFor({ ...idle, checked: false })).toBe('Ainda não verificado')
  })

  it('keeps a found update visible when a later download fails', () => {
    expect(statusLabelFor({ ...idle, available: true, failed: true })).toBe('Atualização disponível')
  })

  it('shows progress over a stale failure while work is running', () => {
    expect(statusLabelFor({ ...idle, failed: true, checking: true })).toBe('Verificando...')
  })
})
