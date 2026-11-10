import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { builtInTabEntries } from './built-in-tab-labels'

export type PaletteItem = {
  id: string
  label: string
  description?: string
  group: string
  action: () => void
}

export function buildItems(
  declarations: FeatureSettingsDeclaration[],
  onSelectTab: (id: string) => void,
  defaultGroup: string,
): PaletteItem[] {
  const items: PaletteItem[] = []
  for (const decl of declarations) {
    items.push({
      id: decl.tabId,
      label: decl.tabLabel,
      description: decl.tabDescription,
      group: decl.tabGroup ?? defaultGroup,
      action: () => onSelectTab(decl.tabId),
    })
    for (const group of decl.groups ?? []) {
      for (const field of group.fields ?? []) {
        items.push({
          id: `${decl.tabId}:${field.configPath}`,
          label: field.label,
          description: `${decl.tabLabel} › ${group.title ?? ''}`,
          group: decl.tabLabel,
          action: () => onSelectTab(decl.tabId),
        })
      }
    }
  }
  // Built-in tabs are not feature declarations; without this the palette never finds them.
  for (const [tabId, label] of builtInTabEntries()) {
    if (items.some((item) => item.id === tabId)) continue
    items.push({ id: tabId, label, group: defaultGroup, action: () => onSelectTab(tabId) })
  }
  return items
}

export function rank(item: PaletteItem, query: string): number {
  const id = item.id.toLowerCase()
  const label = item.label.toLowerCase()
  const isTab = !id.includes(':')
  if (isTab && (id === query || label === query)) return 0
  if (isTab && (id.startsWith(query) || label.startsWith(query))) return 1
  if (isTab) return 2
  return 3
}
