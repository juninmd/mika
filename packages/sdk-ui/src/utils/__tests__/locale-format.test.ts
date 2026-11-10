import { describe, expect, it } from 'vitest'
import { formatLocalDate, formatLocalDuration, formatLocalNumber, formatLocalTime } from '../locale-format'

// The contract is "explicit locale changes the format" — proves pt-BR and en-US produce different output for the same input.

describe('formatLocalDate', () => {
  const SAMPLE = new Date('2026-03-05T00:00:00.000Z')

  it('aceita epoch (number) e Date de forma equivalente', () => {
    expect(formatLocalDate(SAMPLE.getTime(), 'en-US')).toBe(formatLocalDate(SAMPLE, 'en-US'))
  })

  it('locales diferentes produzem formatações diferentes para a mesma data', () => {
    const enUS = formatLocalDate(SAMPLE, 'en-US')
    const ptBR = formatLocalDate(SAMPLE, 'pt-BR')
    expect(enUS).not.toBe(ptBR)
  })

  it('options é repassado ao Intl (ex: weekday long muda a saída)', () => {
    const withWeekday = formatLocalDate(SAMPLE, 'en-US', {
      weekday: 'long',
      year: 'numeric',
      month: 'numeric',
      day: 'numeric',
    })
    const withoutWeekday = formatLocalDate(SAMPLE, 'en-US', { year: 'numeric', month: 'numeric', day: 'numeric' })
    expect(withWeekday).not.toBe(withoutWeekday)
  })
})

describe('formatLocalTime', () => {
  const SAMPLE = new Date('2026-03-05T15:30:00.000Z')

  it('aceita epoch e Date de forma equivalente', () => {
    expect(formatLocalTime(SAMPLE.getTime(), 'en-US')).toBe(formatLocalTime(SAMPLE, 'en-US'))
  })

  it('hour12 muda o formato de 24h para AM/PM', () => {
    const h24 = formatLocalTime(SAMPLE, 'en-US', { hour: '2-digit', minute: '2-digit', hour12: false })
    const h12 = formatLocalTime(SAMPLE, 'en-US', { hour: '2-digit', minute: '2-digit', hour12: true })
    expect(h24).not.toBe(h12)
  })
})

describe('formatLocalNumber', () => {
  it('milhar usa separador diferente entre en-US e pt-BR', () => {
    expect(formatLocalNumber(1234.5, 'en-US')).toContain(',')
    expect(formatLocalNumber(1234.5, 'pt-BR')).toContain('.')
  })

  it('zero e negativo formatam sem lançar', () => {
    expect(formatLocalNumber(0, 'en-US')).toBe('0')
    expect(formatLocalNumber(-5, 'en-US')).toBe('-5')
  })

  it('options controla casas decimais fixas', () => {
    expect(formatLocalNumber(1, 'en-US', { minimumFractionDigits: 2 })).toBe('1.00')
  })
})

describe('formatLocalDuration', () => {
  // "5423s no ar" made the reader do arithmetic; the two largest units say the same thing.
  it('mostra as duas maiores unidades, sem zeros à esquerda', () => {
    expect(formatLocalDuration(5423, 'en-US')).toBe('1h 30m')
    expect(formatLocalDuration(90061, 'en-US')).toBe('1d 1h')
    expect(formatLocalDuration(42, 'en-US')).toBe('42s')
    expect(formatLocalDuration(120, 'en-US')).toBe('2m')
  })

  it('segue o locale e não quebra com entrada inválida', () => {
    expect(formatLocalDuration(5423, 'zh-CN')).not.toBe(formatLocalDuration(5423, 'en-US'))
    expect(formatLocalDuration(Number.NaN, 'en-US')).toBe('0s')
    expect(formatLocalDuration(-5, 'en-US')).toBe('0s')
  })
})
