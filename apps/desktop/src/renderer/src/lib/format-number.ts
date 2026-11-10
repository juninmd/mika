import { getLocale } from '@mika/sdk/i18n'
import { capitalize, formatFixed } from '@mika/sdk/utils/locale-text'

/** `toFixed` in the active UI locale (decimal comma in de/fr/pt/ru/..., dot in en/ja/zh/ko). */
export const fixed = (value: number, digits: number): string => formatFixed(value, digits, getLocale())

/** First letter upper-cased by the active locale's rules. */
export const capitalizeLocal = (text: string): string => capitalize(text, getLocale())
