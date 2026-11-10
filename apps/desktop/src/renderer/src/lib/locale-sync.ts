import { ConfigKey, type SupportedLocale } from '@mika/sdk/contracts'
import { getLocale, isSupportedLocale, setLocale, subscribeLocale, whenLocaleReady } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { rendererLog } from './renderer-logger'
import { getElectronApi } from './shell-api'

// Locale belongs to the process, not the component: reads/listens for config-changed once per window,
// or the preload listener cap (20) would overflow and the next mounted screen might miss the change.

let started = false

// A choice made in this window wins over a saved value still in flight from disk.
let hasLocalSelection = false
let receivedLiveUpdate = false

function applyDocumentLang(): void {
  if (typeof document !== 'undefined') document.documentElement.lang = getLocale()
}

async function loadSavedLocale(): Promise<void> {
  try {
    // Narrow path: getSettingsConfig is denied in plugin windows, taking the locale with it.
    const prefs = await getElectronApi().runtime.getUiPrefs?.()
    if (hasLocalSelection || receivedLiveUpdate || !prefs) return
    if (isSupportedLocale(prefs.locale)) setLocale(prefs.locale)
  } catch (error) {
    rendererLog.warn('[i18n] falha ao ler o idioma salvo', { error: toErrorMessage(error) })
  }
}

// Idempotent: calling from every component is expected; only the first call does anything.
export function ensureLocaleSync(): void {
  if (started) return
  started = true

  subscribeLocale(applyDocumentLang)
  applyDocumentLang()

  let api: ReturnType<typeof getElectronApi>
  try {
    api = getElectronApi()
  } catch {
    return
  }

  void loadSavedLocale()

  // No unsubscribe: the subscription lives for the window's lifetime and there's only one.
  api.runtime.onConfigChanged?.((event) => {
    if (event.path !== ConfigKey.AppLocale || !isSupportedLocale(event.value)) return
    receivedLiveUpdate = true
    setLocale(event.value)
  })
}

// Reverts to the previous locale if the disk write fails, so the UI doesn't lie about what's saved.
export async function changeLocale(next: SupportedLocale): Promise<boolean> {
  const previous = getLocale()
  hasLocalSelection = true
  setLocale(next)
  // Catalogs other than pt-BR/en-US are lazy chunks: resolve once the UI actually shows `next`.
  await whenLocaleReady()
  try {
    const result = await getElectronApi().runtime.saveSetting(ConfigKey.AppLocale, next)
    if (result.success) return true
  } catch {
    // Falls through to the rollback below.
  }
  setLocale(previous)
  return false
}
