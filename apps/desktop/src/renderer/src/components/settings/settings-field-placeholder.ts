import type { SettingsField } from '@mika/sdk/contracts'
import type { Translate } from '../../hooks/useI18n'

/**
 * What the empty input says. The field label already sits above the control, so the generic fallbacks
 * are whole translated sentences — never a label spliced into a Portuguese template.
 */
export function resolvePlaceholder(field: SettingsField, t: Translate): string | undefined {
  if (field.placeholder) return field.placeholder
  if (field.example) return field.example
  if (field.examples?.[0]) return field.examples[0]
  if (field.type === 'number') {
    if (field.min !== undefined) return t('settings.field.placeholder.numberMin', { min: field.min })
    return t('settings.field.placeholder.number')
  }
  if (field.type === 'url') return 'https://...'
  if (field.type === 'password') return t('settings.field.placeholder.password')
  if (field.type === 'textarea') return t('settings.field.placeholder.textarea')
  return t('settings.field.placeholder.text')
}
