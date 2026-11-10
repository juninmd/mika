import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import type { CSSProperties } from 'react'
import { isNamedIcon, resolveNamedIcon } from '../Icons'

type PluginAccentStyle = CSSProperties &
  Record<
    | '--plugin-accent'
    | '--plugin-accent-soft'
    | '--plugin-accent-border'
    | '--plugin-accent-glow'
    | '--plugin-accent-text',
    string
  >

const ACCENT_SHIFTS: Record<string, number> = {
  violet: 0,
  indigo: -14,
  sky: -28,
  cyan: -42,
  teal: -35,
  emerald: -21,
  green: -21,
  lime: -7,
  yellow: 7,
  amber: 14,
  orange: 21,
  rose: 28,
  pink: 35,
  fuchsia: 42,
  slate: 0,
  gray: 0,
}

const DEFAULT_SHIFT = 0

function pluginAccentStyle(shift: number): PluginAccentStyle {
  const h = `calc(var(--accent-h) + ${shift})`
  return {
    '--plugin-accent': `hsl(${h} 85% 62%)`,
    '--plugin-accent-soft': `hsl(${h} 85% 62% / 0.15)`,
    '--plugin-accent-border': `hsl(${h} 85% 62% / 0.32)`,
    '--plugin-accent-glow': `hsl(${h} 85% 62% / 0.24)`,
    '--plugin-accent-text': `hsl(${h} 85% 80%)`,
  }
}

type PluginVisual = {
  fieldCount: number
  groupCount: number
  iconName: string
  initials: string
  showMonogram: boolean
  style: PluginAccentStyle
}

export function resolvePluginVisual(
  tab: Pick<FeatureSettingsDeclaration, 'tabId' | 'tabLabel' | 'tabIcon' | 'tabGroup' | 'accentColor' | 'groups'>,
): PluginVisual {
  const shift = ACCENT_SHIFTS[(tab.accentColor ?? '').toLowerCase()] ?? DEFAULT_SHIFT
  const hasNamedIcon = isNamedIcon(tab.tabIcon)
  const iconName = hasNamedIcon && tab.tabIcon ? tab.tabIcon : 'Spark'
  const initials = getPluginInitials(tab)

  return {
    fieldCount: tab.groups.reduce((sum, group) => sum + group.fields.length, 0),
    groupCount: tab.groups.length,
    iconName,
    initials,
    showMonogram: !hasNamedIcon,
    style: pluginAccentStyle(shift),
  }
}

export function PluginGlyph({ iconName, className }: { iconName: string; className?: string }) {
  const Icon = resolveNamedIcon(iconName)
  return <Icon className={className} />
}

function getPluginInitials(tab: Pick<FeatureSettingsDeclaration, 'tabLabel' | 'tabIcon'>): string {
  const customMark = tab.tabIcon?.trim()
  if (customMark && customMark.length <= 3 && customMark.toUpperCase() === customMark) return customMark

  const plain = stripDiacritics(tab.tabLabel)
    .replace(/[^A-Za-z0-9 ]+/g, ' ')
    .trim()
  if (!plain) return 'MI'

  const words = plain.split(/\s+/).filter(Boolean)
  if (words.length >= 2) return `${words[0][0]}${words[1][0]}`.toUpperCase()
  return plain.slice(0, 2).toUpperCase()
}

function stripDiacritics(value: string): string {
  return value.normalize('NFD').replace(/[\u0300-\u036f]/g, '')
}
