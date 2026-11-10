import type { TranslationDict } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { type LucideIcon, Sparkles, Wrench } from 'lucide-react'
import type { ElectronShellApi, ToolCatalogItem } from '../../../shared/electron-api-types'
import { MATCH_EFFECT_IDS, matchEffectLabel } from '../lib/match-effect'
import type { useUIStore } from '../store/useUIStore'

export interface PaletteItem {
  id: string
  label: string
  description?: string
  icon: LucideIcon
  group: string
  onSelect: () => void
}

/** A group whose entries all repeat one description (the "Effect: X" rows) says it once, in the header. */
export function sharedGroupDescription(items: PaletteItem[]): string | undefined {
  const first = items[0]?.description
  return items.length >= 3 && first && items.every((item) => item.description === first) ? first : undefined
}

function fuzzyScore(query: string, target: string): number {
  if (!query) return 1
  const q = query.toLowerCase()
  const t = target.toLowerCase()
  if (t.startsWith(q)) return 3
  if (t.includes(q)) return 2
  let qi = 0
  let score = 0
  let consecutive = 0
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] === q[qi]) {
      qi++
      consecutive++
      score += consecutive
    } else {
      consecutive = 0
    }
  }
  return qi === q.length ? score / q.length : 0
}

const GROUP_ORDER: ReadonlyArray<keyof TranslationDict> = [
  'shell.palette.group.navigation',
  'shell.palette.group.effects',
  'shell.palette.group.tools',
  'shell.palette.group.system',
]

function humanizeToolName(name: string): string {
  return name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/_/g, ' ')
    .toLowerCase()
}

function toolPaletteEntries(t: ToolCatalogItem): { suffix: string; label: string; args: Record<string, unknown> }[] {
  const required = t.inputSchema?.required ?? []
  if (required.length === 0) return [{ suffix: '', label: humanizeToolName(t.name), args: {} }]
  if (required.length !== 1 || required[0] !== 'action') return []
  const actions = t.inputSchema?.properties?.action?.enum ?? []
  return actions.map((action) => ({
    suffix: `:${action}`,
    label: `${humanizeToolName(t.name)} · ${humanizeToolName(action)}`,
    args: { action },
  }))
}

export function toolFailureMessage(
  t: (k: 'shell.palette.toolError') => string,
  label: string,
  detail?: string,
): string {
  const clean = detail?.trim()
  const base = t('shell.palette.toolError').replace('{{label}}', label)
  return clean ? `${base}: ${clean}` : `${base}.`
}

export interface PaletteDeps {
  t: (key: keyof TranslationDict) => string
  eAPI: ElectronShellApi
  close: () => void
  addNotification: ReturnType<typeof useUIStore.getState>['addNotification']
}

export function buildToolItems(
  tools: ToolCatalogItem[],
  { t, eAPI, close, addNotification }: PaletteDeps,
): PaletteItem[] {
  return tools.flatMap((tool) =>
    toolPaletteEntries(tool).map((entry) => ({
      id: `tool:${tool.name}${entry.suffix}`,
      label: entry.label,
      description: tool.description,
      icon: Wrench,
      group: t('shell.palette.group.tools'),
      onSelect: () => {
        close()
        eAPI?.runtime
          ?.executeTool?.(tool.name, entry.args)
          .then((res) => {
            if (res && typeof res === 'object' && 'success' in res && res.success === false) {
              addNotification('error', toolFailureMessage(t, entry.label, (res as { error?: string }).error), tool.name)
            }
          })
          .catch((err: unknown) => {
            addNotification('error', toolFailureMessage(t, entry.label, toDisplayError(err)), tool.name)
          })
      },
    })),
  )
}

export function buildEffectItems(
  tools: ToolCatalogItem[],
  { t, eAPI, close, addNotification }: PaletteDeps,
): PaletteItem[] {
  const hasMatchEffect = tools.some((tool) => tool.name === 'match_effect')
  return hasMatchEffect
    ? MATCH_EFFECT_IDS.map((id) => {
        const fx = { id, label: matchEffectLabel(id, t) }
        const effectLabel = `${t('shell.palette.effect')}: ${fx.label}`
        const effectFailureLabel = `${t('shell.palette.effect').toLowerCase()} ${fx.label}`
        return {
          id: `effect:${fx.id}`,
          label: effectLabel,
          description: t('shell.palette.effectHint'),
          icon: Sparkles,
          group: t('shell.palette.group.effects'),
          onSelect: () => {
            close()
            eAPI?.runtime
              ?.executeTool?.('match_effect', { text: `/${fx.id}` })
              .then((res) => {
                if (res && typeof res === 'object' && 'success' in res && res.success === false) {
                  addNotification(
                    'error',
                    toolFailureMessage(t, effectFailureLabel, (res as { error?: string }).error),
                    'match_effect',
                  )
                }
              })
              .catch((err: unknown) => {
                addNotification('error', toolFailureMessage(t, effectFailureLabel, toDisplayError(err)), 'match_effect')
              })
          },
        }
      })
    : []
}

export function groupPaletteItems(allItems: PaletteItem[], query: string, t: PaletteDeps['t']) {
  const list: Array<{ group: string; items: PaletteItem[] }> = []
  if (!query) {
    for (const key of GROUP_ORDER) {
      const g = t(key)
      const items = allItems.filter((i) => i.group === g)
      if (items.length) list.push({ group: g, items })
    }
  } else {
    const scored = allItems
      .map((item) => {
        const s1 = fuzzyScore(query, item.label)
        const s2 = fuzzyScore(query, item.description ?? '') * 0.5
        return { item, score: Math.max(s1, s2) }
      })
      .filter((x) => x.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 50)
    list.push({ group: '', items: scored.map((x) => x.item) })
  }

  return {
    grouped: list,
    flat: list.flatMap((g) => g.items),
  }
}
