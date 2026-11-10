import { ConfigKey } from '@mika/sdk/contracts'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const saveSetting = vi.fn().mockResolvedValue({ success: true })
const quit = vi.fn().mockResolvedValue(undefined)

vi.mock('../shell-api', () => ({
  getElectronApi: () => ({ runtime: { saveSetting }, shell: { system: { quit } } }),
}))

const { isLegalAccepted, saveLegalAcceptance, quitApp } = await import('../legal-acceptance')

beforeEach(() => {
  saveSetting.mockClear()
  quit.mockClear()
})

describe('isLegalAccepted', () => {
  it.each([
    [
      'a versão aceita é a atual e a idade foi confirmada',
      { legal: { acceptedVersion: 'v2', ageConfirmed: true } },
      'v2',
      true,
    ],
    [
      'versão atual mas sem a confirmação de idade (aceite de antes do gate de idade)',
      { legal: { acceptedVersion: 'v2' } },
      'v2',
      false,
    ],
    ['idade não confirmada explicitamente', { legal: { acceptedVersion: 'v2', ageConfirmed: false } }, 'v2', false],
    ['o documento mudou desde o aceite', { legal: { acceptedVersion: 'v1', ageConfirmed: true } }, 'v2', false],
    ['config de quem já existia antes do aceite (sem namespace)', {}, 'v2', false],
    ['default do schema (vazio)', { legal: { acceptedVersion: '' } }, 'v2', false],
    [
      'versão ainda desconhecida não conta como aceite, nem com campo vazio',
      { legal: { acceptedVersion: '' } },
      '',
      false,
    ],
    ['valor que não é texto', { legal: { acceptedVersion: 2 } }, '2', false],
  ])('%s', (_name, config, current, expected) => {
    expect(isLegalAccepted(config, current)).toBe(expected)
  })
})

describe('saveLegalAcceptance', () => {
  it('grava a versão aceita e o instante do aceite, nas chaves do schema Rust', async () => {
    const results = await saveLegalAcceptance('v2', new Date('2026-10-06T12:00:00.000Z'))

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LegalAcceptedVersion, 'v2')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LegalAcceptedAt, '2026-10-06T12:00:00.000Z')
    // Aceitar exige marcar "18 anos ou mais": os dois valores vão juntos, nunca um sem o outro.
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LegalAgeConfirmed, true)
    expect(ConfigKey.LegalAgeConfirmed).toBe('legal.ageConfirmed')
    expect(ConfigKey.LegalAcceptedVersion).toBe('legal.acceptedVersion')
    expect(ConfigKey.LegalAcceptedAt).toBe('legal.acceptedAt')
    expect(results).toHaveLength(3)
  })
})

describe('quitApp', () => {
  it('encerra pelo comando de shell do app, sem fechar só a janela', async () => {
    await quitApp()

    expect(quit).toHaveBeenCalledOnce()
  })
})
