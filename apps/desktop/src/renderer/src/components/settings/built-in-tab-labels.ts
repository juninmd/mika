import { type TranslationDict, t } from '@mika/sdk/i18n'

/** Chave do rótulo das abas embutidas — as que não vêm de declaração de feature. */
const BUILT_IN_TAB_LABEL_KEYS: Record<string, keyof TranslationDict> = {
  about: 'settings.tab.about',
  'audio-devices': 'settings.tab.audioDevices',
  backup: 'settings.tab.backup',
  credentials: 'settings.tab.credentials',
  homologation: 'settings.tab.homologation',
  logs: 'settings.tab.logs',
  modules: 'settings.tab.modules',
  overview: 'settings.tab.overview',
  prompts: 'settings.tab.prompts',
  skin: 'settings.tab.skin',
  'tools-mcp': 'settings.tab.toolsMcp',
  updates: 'settings.tab.updates',
}

/** Rótulo localizado de uma aba embutida (resolvido a cada chamada: acompanha o idioma ativo). */
export function builtInTabLabel(tabId: string): string | undefined {
  const key = BUILT_IN_TAB_LABEL_KEYS[tabId]
  return key ? t(key) : undefined
}

export function builtInTabEntries(): Array<[tabId: string, label: string]> {
  return Object.entries(BUILT_IN_TAB_LABEL_KEYS).map(([tabId, key]) => [tabId, t(key)])
}
