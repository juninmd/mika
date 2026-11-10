import type { SettingsField } from '@mika/sdk/contracts'
import { t } from '@mika/sdk/i18n'

export function formatValue(field: SettingsField, value: unknown): string {
  if (value === undefined || value === null) return ''
  if (field.type === 'number') return String(value)
  return String(value)
}

export function parseValue(field: SettingsField, draft: string, currentValue: unknown): unknown {
  if (field.type === 'number') return draft === '' ? currentValue : Number(draft)
  return draft
}

function validateNumberField(value: unknown, min?: number, max?: number): string | null {
  const numericValue = Number(value)
  if (Number.isNaN(numericValue)) return t('settings.validation.numberInvalid')
  if (min !== undefined && numericValue < min) return t('settings.validation.numberMin').replace('{{min}}', String(min))
  if (max !== undefined && numericValue > max) return t('settings.validation.numberMax').replace('{{max}}', String(max))
  return null
}

function validateTextField(
  value: unknown,
  minLength?: number,
  maxLength?: number,
  pattern?: string,
  patternError?: string,
): string | null {
  const text = String(value ?? '')
  if (minLength !== undefined && text.length < minLength)
    return t('settings.validation.minLength').replace('{{min}}', String(minLength))
  if (maxLength !== undefined && text.length > maxLength)
    return t('settings.validation.maxLength').replace('{{max}}', String(maxLength))
  if (!pattern) return null

  try {
    const regex = new RegExp(pattern)
    if (text && !regex.test(text)) return patternError ?? t('settings.validation.patternDefault')
  } catch {
    return null
  }

  return null
}

export function validateField(field: SettingsField, value: unknown): string | null {
  if (field.type === 'number') return validateNumberField(value, field.min, field.max)
  return validateTextField(value, field.minLength, field.maxLength, field.pattern, field.patternError)
}
