import type { TranslationDict } from '@mika/sdk/i18n'
import { SPRING } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { Cpu, Gamepad2, KeyRound, Mic, Settings, Sparkles, Wrench } from 'lucide-react'
import { type Translate, useI18n } from '../../hooks/useI18n'
import { handleNavKeyDown } from './nav-rail-keyboard'

export interface NavSection {
  id: string
  labelKey: keyof TranslationDict
  icon: React.ComponentType<{ size?: number; strokeWidth?: number; className?: string }>
  tabIds: string[]
}

export const MIKA_NAV_SECTIONS: NavSection[] = [
  {
    id: 'mika',
    labelKey: 'shell.settingsNav.section.mika',
    icon: Sparkles,
    tabIds: ['character', 'skin', 'mood', 'user'],
  },
  {
    id: 'cognicao',
    labelKey: 'shell.settingsNav.section.cognicao',
    icon: Cpu,
    tabIds: ['llm', 'embeddings', 'brain', 'vision', 'prompts'],
  },
  { id: 'voz', labelKey: 'shell.settingsNav.section.voz', icon: Mic, tabIds: ['voice', 'stt', 'audio-devices'] },
  {
    id: 'interacao',
    labelKey: 'shell.settingsNav.section.interacao',
    icon: Gamepad2,
    tabIds: ['gamer-companion', 'person-catalog'],
  },
  {
    id: 'ferramentas',
    labelKey: 'shell.settingsNav.section.ferramentas',
    icon: Wrench,
    tabIds: ['mcp', 'browser', 'scheduler', 'tools-mcp', 'computer-use'],
  },
  {
    id: 'conta',
    labelKey: 'shell.settingsNav.section.conta',
    icon: KeyRound,
    tabIds: ['credentials', 'privacy', 'backup'],
  },
  {
    id: 'sistema',
    labelKey: 'shell.settingsNav.section.sistema',
    icon: Settings,
    tabIds: ['overview', 'modules', 'homologation', 'logs', 'updates', 'about'],
  },
]

export const BUILT_IN_TAB_IDS = new Set([
  'about',
  'audio-devices',
  'backup',
  'credentials',
  'homologation',
  'logs',
  'modules',
  'overview',
  'prompts',
  'skin',
  'tools-mcp',
  'updates',
])

export { builtInTabLabel } from './built-in-tab-labels'

export { SubTabBar } from './SubTabBar'

/** A seção de um `tabId` — resposta única, não duplicada com fallback diferente em `SettingsApp.tsx`. */
export function sectionForTab(tabId: string): NavSection {
  return (
    MIKA_NAV_SECTIONS.find((s) => s.tabIds.includes(tabId)) ??
    (MIKA_NAV_SECTIONS[MIKA_NAV_SECTIONS.length - 1] as NavSection)
  )
}

interface Props {
  activeTabId: string | null
  declarations: Array<{ tabId: string; tabLabel: string; tabGroup?: string }>
  onSelectTab: (tabId: string) => void
  health: { status: string; tools?: number } | null
}

/** Aba viva de uma seção: embutida ou declarada por feature. Seção sem nenhuma não é desenhada. */
export function hasReachableTab(section: NavSection, declarations: ReadonlyArray<{ tabId: string }>): boolean {
  return section.tabIds.some((tid) => BUILT_IN_TAB_IDS.has(tid) || declarations.some((d) => d.tabId === tid))
}

/** O que a seção abre ao ser clicada: a primeira aba viva, na ordem declarada. */
function firstReachableTab(section: NavSection, declarations: ReadonlyArray<{ tabId: string }>): string {
  return (
    section.tabIds.find((tid) => BUILT_IN_TAB_IDS.has(tid) || declarations.some((d) => d.tabId === tid)) ??
    section.tabIds[0] ??
    'overview'
  )
}

function renderSectionButton(
  section: NavSection,
  declarations: ReadonlyArray<{ tabId: string }>,
  isActive: boolean,
  onSelectTab: (tabId: string) => void,
  t: Translate,
) {
  if (section.tabIds.length > 0 && !hasReachableTab(section, declarations)) return null
  const Icon = section.icon
  const label = t(section.labelKey)
  return (
    <button
      key={section.id}
      type="button"
      aria-label={label}
      aria-current={isActive ? 'page' : undefined}
      onClick={() => onSelectTab(firstReachableTab(section, declarations))}
      className={`group relative flex flex-col items-center gap-1 rounded-xl px-1 py-2 transition duration-200 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${isActive ? 'text-accent-text' : 'hover:bg-surface-raise/50'}`}
    >
      {isActive && (
        // Transform/opacity only. A shared `layoutId` indicator made framer-motion measure the rail
        // (getBoundingClientRect) around every commit that re-rendered it — a tab switch mounting a
        // heavy panel paid a forced synchronous layout of the whole new page for a highlight.
        <motion.div
          aria-hidden="true"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          className="absolute inset-0 rounded-xl border border-accent-rim bg-accent-soft"
          transition={SPRING.snappy}
        />
      )}
      <Icon
        size={18}
        strokeWidth={isActive ? 2 : 1.5}
        aria-hidden="true"
        className={isActive ? '' : 'text-text-muted group-hover:text-text-main transition-colors'}
      />
      <span
        className={`text-[12px] font-bold uppercase leading-tight text-center break-words [overflow-wrap:anywhere] transition-colors ${isActive ? '' : 'text-text-muted group-hover:text-text-main'}`}
      >
        {label}
      </span>
    </button>
  )
}

export function MikaNavRail({ activeTabId, declarations, onSelectTab, health }: Props) {
  const { t } = useI18n()
  const activeSection = activeTabId ? sectionForTab(activeTabId) : MIKA_NAV_SECTIONS[MIKA_NAV_SECTIONS.length - 1]

  return (
    <nav
      aria-label={t('shell.settingsNav.ariaLabel')}
      onKeyDown={handleNavKeyDown}
      className="relative z-10 flex flex-col border-r border-border-subtle w-[136px] py-3 px-1.5 gap-0.5 shrink-0"
    >
      <div className="mb-3 flex justify-center">
        <div
          aria-hidden="true"
          className="flex h-8 w-8 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft"
        >
          <span className="text-caption font-black tracking-tight text-accent">M</span>
        </div>
      </div>

      {MIKA_NAV_SECTIONS.map((section) =>
        renderSectionButton(section, declarations, activeSection?.id === section.id, onSelectTab, t),
      )}

      <div className="flex-1" />

      <div className="flex justify-center pb-1">
        <div
          role="status"
          aria-label={
            health?.status === 'degraded'
              ? t('shell.settingsNav.bootDegraded')
              : health?.status === 'ok'
                ? t('shell.settingsNav.bootHealthy')
                : t('shell.settingsNav.bootInProgress')
          }
          title={
            health?.status === 'degraded'
              ? t('shell.settingsNav.bootDegraded')
              : health?.status === 'ok'
                ? t('shell.settingsNav.bootHealthy')
                : t('shell.settingsNav.bootInProgress')
          }
          className={`h-1.5 w-1.5 rounded-full ${
            health?.status === 'ok' ? 'bg-ok' : health?.status === 'degraded' ? 'bg-warn' : 'bg-text-subtle'
          }`}
        />
      </div>
    </nav>
  )
}
