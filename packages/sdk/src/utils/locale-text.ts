/** `Number#toFixed` with the locale's decimal separator (always exactly `digits` decimals). */
export function formatFixed(value: number, digits: number, locale?: string): string {
  return value.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits })
}

/** First character upper-cased by locale rules; splits by code point so an emoji/astral first char stays whole. */
export function capitalize(text: string, locale?: string): string {
  const [first = '', ...rest] = Array.from(text)
  return first.toLocaleUpperCase(locale) + rest.join('')
}
