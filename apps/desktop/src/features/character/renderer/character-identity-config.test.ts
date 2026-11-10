import { describe, expect, it, vi } from 'vitest'

const active = vi.hoisted(() => ({ locale: 'pt-BR' }))
vi.mock('@mika/sdk/i18n', () => ({ LOCALE_OPTIONS: [], getLocale: () => active.locale }))

const { resolveIdentityConfig } = await import('./character-identity-config')

const onSave = async () => {}

describe('resolveIdentityConfig', () => {
  // The settings host passes only `config`: reading the relationship from props alone
  // pinned the selector to 'friend' and the saved choice never showed up.
  it('reads the saved relationship and affinity from config', () => {
    const r = resolveIdentityConfig({
      onSave,
      config: { character: { relationshipStatus: 'companion', affinityScore: 42 } },
    })
    expect(r.relationshipStatus).toBe('companion')
    expect(r.affinityScore).toBe(42)
  })

  it('falls back to the schema defaults when unset', () => {
    const r = resolveIdentityConfig({ onSave, config: {} })
    expect(r.relationshipStatus).toBe('friend')
    expect(r.affinityScore).toBe(0)
  })

  it('normalizes an unknown stored value to friend', () => {
    const r = resolveIdentityConfig({ onSave, config: { character: { relationshipStatus: 'girlfriend' } } })
    expect(r.relationshipStatus).toBe('friend')
  })

  // Sem `app.locale` salvo (perfil novo) o seletor de idioma mostra o idioma que a tela está falando.
  it('shows the active interface language when app.locale is unset, and the passed one when set', () => {
    active.locale = 'de-DE'
    expect(resolveIdentityConfig({ onSave }).cfg['app.locale']).toBe('de-DE')
    expect(resolveIdentityConfig({ onSave, appLocale: 'ja-JP' }).cfg['app.locale']).toBe('ja-JP')
    active.locale = 'pt-BR'
  })
})
