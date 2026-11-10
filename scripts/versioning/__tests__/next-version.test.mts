import { describe, expect, it } from 'vitest'
import { nextVersion } from '../next-version.mts'

describe('nextVersion — o commit convencional decide o salto', () => {
  it('sem commit novo desde a tag não há release: push repetido não gera versão vazia', () => {
    expect(nextVersion('v0.6.0', [])).toBeNull()
  })

  it('fix, chore e merge sobem o patch', () => {
    expect(nextVersion('v0.6.0', ['fix(llm): x', 'chore: y', "Merge branch 'a'"])).toBe('0.6.1')
  })

  it('feat sobe o minor e zera o patch', () => {
    expect(nextVersion('v0.6.3', ['fix: a', 'feat(voice): b'])).toBe('0.7.0')
  })

  it('quebra de contrato em 0.x sobe o minor: antes do 1.0 o major não é promessa', () => {
    expect(nextVersion('v0.6.3', ['feat!: nova api'])).toBe('0.7.0')
    expect(nextVersion('v0.6.3', ['fix: a\n\nBREAKING CHANGE: remove b'])).toBe('0.7.0')
  })

  it('quebra de contrato depois do 1.0 sobe o major', () => {
    expect(nextVersion('v1.4.2', ['refactor(sdk)!: renomeia token'])).toBe('2.0.0')
  })

  it('tag fora do formato vX.Y.Z é erro, não um palpite de versão', () => {
    expect(() => nextVersion('release-7', ['fix: a'])).toThrow(/vX\.Y\.Z/)
  })
})
