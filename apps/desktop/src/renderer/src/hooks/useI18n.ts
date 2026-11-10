import type { TranslateParams, TranslationDict } from '@mika/sdk/i18n'
import { getLocale, subscribeLocale, t } from '@mika/sdk/i18n'
import { useCallback, useEffect, useMemo, useSyncExternalStore } from 'react'
import { changeLocale, ensureLocaleSync } from '../lib/locale-sync'

export type Translate = {
  (key: keyof TranslationDict): string
  /** `count` escolhe a forma de plural do idioma; as demais variáveis trocam `{{nome}}`. */
  (key: keyof TranslationDict, params: TranslateParams): string
}

// Disk read and change listening live in `locale-sync` once per window — 89 callers each opening its own IPC subscription blew the preload listener cap.
export function useI18n() {
  const locale = useSyncExternalStore(subscribeLocale, getLocale, getLocale)

  useEffect(ensureLocaleSync, [])

  const translate = useCallback<Translate>(
    (key: keyof TranslationDict, params?: TranslateParams) => (params ? t(key, params, locale) : t(key, locale)),
    [locale],
  )

  return useMemo(() => ({ t: translate, locale, setLocale: changeLocale }), [translate, locale])
}
