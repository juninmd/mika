import type { SettingsField } from '@mika/sdk/contracts'
import { type TranslateParams, type TranslationDict, t as translate } from '@mika/sdk/i18n'
import { describe, expect, it } from 'vitest'
import type { Translate } from '../../../hooks/useI18n'
import { resolvePlaceholder } from '../settings-field-placeholder'

const t = ((key: keyof TranslationDict, params?: TranslateParams) =>
  params ? translate(key, params) : translate(key)) as Translate

const field = (over: Partial<SettingsField>): SettingsField =>
  ({ configPath: 'x.y', label: 'Taxa', type: 'text', ...over }) as SettingsField

describe('resolvePlaceholder', () => {
  it('prefers what the declaration provides', () => {
    expect(resolvePlaceholder(field({ placeholder: 'abc' }), t)).toBe('abc')
    expect(resolvePlaceholder(field({ example: 'ex' }), t)).toBe('ex')
    expect(resolvePlaceholder(field({ examples: ['first', 'second'] }), t)).toBe('first')
  })

  // Regression: the fallback was `Informe ${label.toLowerCase()}` — Portuguese grammar glued to a translated
  // label, which reads wrong in German (nouns are capitalized) and was never localized at all.
  it('never splices the field label into a sentence', () => {
    const a = resolvePlaceholder(field({ label: 'Chave de API', type: 'password' }), t)
    const b = resolvePlaceholder(field({ label: 'Schlüssel', type: 'password' }), t)
    expect(a).toBe(b)
    expect(a).toBe(t('settings.field.placeholder.password'))
    expect(a?.toLowerCase()).not.toContain('chave de api')
  })

  it('numbers show the minimum when there is one, a generic hint otherwise', () => {
    expect(resolvePlaceholder(field({ type: 'number', min: 3 }), t)).toBe(
      t('settings.field.placeholder.numberMin', { min: 3 }),
    )
    expect(resolvePlaceholder(field({ type: 'number' }), t)).toBe(t('settings.field.placeholder.number'))
  })

  it('urls keep the neutral scheme hint and text/textarea get their own generic copy', () => {
    expect(resolvePlaceholder(field({ type: 'url' }), t)).toBe('https://...')
    expect(resolvePlaceholder(field({ type: 'textarea' }), t)).toBe(t('settings.field.placeholder.textarea'))
    expect(resolvePlaceholder(field({ type: 'text' }), t)).toBe(t('settings.field.placeholder.text'))
  })
})
