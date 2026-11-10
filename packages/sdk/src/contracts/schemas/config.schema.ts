import { z } from 'zod'

export const SUPPORTED_LOCALES = [
  'en-US',
  'pt-BR',
  'es-ES',
  'fr-FR',
  'it-IT',
  'ja-JP',
  'zh-CN',
  'ko-KR',
  'ru-RU',
  'de-DE',
] as const

export const SupportedLocaleSchema = z.enum(SUPPORTED_LOCALES)
export type SupportedLocale = z.infer<typeof SupportedLocaleSchema>

export type AppConfig = Record<string, unknown>
