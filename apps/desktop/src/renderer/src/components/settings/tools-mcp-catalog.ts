import { getLocale, type TranslationDict } from '@mika/sdk/i18n'
import type { ToolCatalogItem } from '../../lib/shell-api'

export type TKey = keyof TranslationDict

export const PLUGIN_LABEL_KEYS: Record<string, TKey> = {
  core: 'settings.toolsMcp.plugin.core',
  'mika.brain': 'settings.toolsMcp.plugin.brain',
  'mika.chat': 'settings.toolsMcp.plugin.chat',
  'mika.vision': 'settings.toolsMcp.plugin.vision',
  'mika.voice': 'settings.toolsMcp.plugin.voice',
  'mika.character': 'settings.toolsMcp.plugin.character',
  'mika.desktop': 'settings.toolsMcp.plugin.desktop',
  'mika.embeddings': 'settings.toolsMcp.plugin.embeddings',
  'mika.discord': 'settings.toolsMcp.plugin.discord',
  'mika.files': 'settings.toolsMcp.plugin.files',
  'mika.gamer-companion': 'settings.toolsMcp.plugin.gamerCompanion',
  'mika.github': 'settings.toolsMcp.plugin.github',
  'mika.mcp': 'settings.toolsMcp.plugin.mcp',
  'mika.mood': 'settings.toolsMcp.plugin.mood',
  'mika.obs': 'settings.toolsMcp.plugin.obs',
  'mika.person-catalog': 'settings.toolsMcp.plugin.personCatalog',
  'mika.privacy': 'settings.toolsMcp.plugin.privacy',
  'mika.scheduler': 'settings.toolsMcp.plugin.scheduler',
  'mika.telegram': 'settings.toolsMcp.plugin.telegram',
  'mika.twitch': 'settings.toolsMcp.plugin.twitch',
  'mika.user': 'settings.toolsMcp.plugin.user',
}

export type ExecEntry = { uid: number; name: string; success: boolean; durationMs?: number; error?: string }

export function extractToolError(result: unknown): string | undefined {
  if (typeof result !== 'object' || result === null) return undefined
  const envelope = result as { error?: unknown }
  return typeof envelope.error === 'string' ? envelope.error : undefined
}

export const MCP_PLUGIN_ID = 'mika.mcp'

// Rust registers some tools as `vision`/`voice` and others as `mika.voice`; one feature must be one group.
function pluginKey(pluginId: string | undefined): string {
  if (!pluginId || pluginId === 'core') return 'core'
  return pluginId.startsWith('mika.') ? pluginId : `mika.${pluginId}`
}

export function groupByPlugin(tools: ToolCatalogItem[]): Array<[string, ToolCatalogItem[]]> {
  const map = new Map<string, ToolCatalogItem[]>()
  for (const tool of tools) {
    const key = pluginKey(tool.pluginId)
    const bucket = map.get(key) ?? []
    bucket.push(tool)
    map.set(key, bucket)
  }
  return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0], getLocale()))
}
