import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { StatusPill } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { type KeyboardEvent, memo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { ModuleToggleButton, ModuleValidationButton } from './ModuleCardActions'
import { PluginGlyph, resolvePluginVisual } from './plugin-visuals'
import { useFeatureValidation } from './useFeatureValidation'

type ModuleCardProps = {
  active: boolean
  enabled: boolean
  fieldCount: number
  index: number
  saving: boolean
  tab: FeatureSettingsDeclaration
  onSelect: (tabId: string) => void
  onToggle?: (tabId: string, next: boolean) => void
}

function ModuleCardInner({ active, enabled, fieldCount, index, saving, tab, onSelect, onToggle }: ModuleCardProps) {
  const { t } = useI18n()
  const visual = resolvePluginVisual(tab)
  const validation = useFeatureValidation(tab)
  const selectFromKeyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || (event.key !== 'Enter' && event.key !== ' ')) return
    event.preventDefault()
    onSelect(tab.tabId)
  }

  return (
    <motion.div
      role="button"
      tabIndex={0}
      aria-label={t('settings.moduleCard.open', { label: tab.tabLabel })}
      aria-pressed={active}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.02 }}
      onClick={() => onSelect(tab.tabId)}
      onKeyDown={selectFromKeyboard}
      style={visual.style}
      className={`group relative cursor-pointer overflow-hidden rounded-xl border p-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
        enabled
          ? 'border-border-subtle bg-surface-raise hover:border-border-strong hover:bg-surface-raise-strong/80'
          : // Off is told by the neutral icon and the switch; fading the card also faded the switch that turns it on.
            'border-border-subtle bg-surface-raise/30 hover:border-border-strong'
      } ${active ? 'ring-2 ring-focus/50 border-border-strong' : ''}`}
    >
      <div className="relative flex items-center justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3.5">
          <ModuleIcon enabled={enabled} visual={visual} />
          <ModuleSummary fieldCount={fieldCount} tab={tab} />
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <ModuleValidationButton label={tab.tabLabel} validation={validation} />
          {onToggle && (
            <ModuleToggleButton
              enabled={enabled}
              label={tab.tabLabel}
              saving={saving}
              onToggle={() => onToggle(tab.tabId, !enabled)}
            />
          )}
        </div>
      </div>
    </motion.div>
  )
}

function areModuleCardsEqual(prev: ModuleCardProps, next: ModuleCardProps): boolean {
  if (
    prev.active !== next.active ||
    prev.enabled !== next.enabled ||
    prev.fieldCount !== next.fieldCount ||
    prev.index !== next.index ||
    prev.saving !== next.saving ||
    prev.onSelect !== next.onSelect ||
    prev.onToggle !== next.onToggle
  ) {
    return false
  }
  if (prev.tab === next.tab) return true
  const a = prev.tab
  const b = next.tab
  return (
    a.tabId === b.tabId &&
    a.tabLabel === b.tabLabel &&
    a.tabDescription === b.tabDescription &&
    a.tabIcon === b.tabIcon &&
    a.tabGroup === b.tabGroup &&
    a.accentColor === b.accentColor &&
    a.isCore === b.isCore &&
    a.toolCount === b.toolCount &&
    a.pluginId === b.pluginId &&
    a.component === b.component &&
    a.enabledConfigPath === b.enabledConfigPath
  )
}

export const ModuleCard = memo(ModuleCardInner, areModuleCardsEqual)

function ModuleIcon({ enabled, visual }: { enabled: boolean; visual: ReturnType<typeof resolvePluginVisual> }) {
  return (
    <div
      aria-hidden="true"
      className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-lg border transition-transform duration-300 group-hover:scale-105 ${
        enabled
          ? 'border-[var(--plugin-accent-border)] bg-[var(--plugin-accent-soft)] text-[var(--plugin-accent-text)]'
          : 'border-border-subtle bg-surface-raise text-text-muted'
      }`}
    >
      {visual.showMonogram ? (
        <span className="text-caption font-black uppercase tracking-widest">{visual.initials}</span>
      ) : (
        <PluginGlyph iconName={visual.iconName} className="h-5 w-5" />
      )}
    </div>
  )
}

function ModuleSummary({ fieldCount, tab }: { fieldCount: number; tab: FeatureSettingsDeclaration }) {
  const { t } = useI18n()
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="truncate text-[13px] font-bold text-text-main" title={tab.tabLabel}>
          {tab.tabLabel}
        </span>
      </div>
      <p
        title={tab.tabDescription || undefined}
        className="mt-0.5 line-clamp-2 text-caption leading-snug text-text-muted"
      >
        {tab.tabDescription || t('settings.modules.card.defaultDescription')}
      </p>
      <ModuleBadges fieldCount={fieldCount} tab={tab} />
    </div>
  )
}

function ModuleBadges({ fieldCount, tab }: { fieldCount: number; tab: FeatureSettingsDeclaration }) {
  const { t } = useI18n()
  const count = (
    n: number,
    one: 'settings.modules.card.field' | 'settings.modules.card.tool',
    many: 'settings.modules.card.fields' | 'settings.modules.card.tools',
  ) => (n === 1 ? t(one) : t(many).replace('{{count}}', String(n)))
  return (
    <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
      <StatusPill
        variant="neutral"
        label={count(fieldCount, 'settings.modules.card.field', 'settings.modules.card.fields')}
      />
      {tab.toolCount ? (
        <StatusPill
          variant="neutral"
          label={count(tab.toolCount, 'settings.modules.card.tool', 'settings.modules.card.tools')}
        />
      ) : null}
      {!tab.enabledConfigPath && <StatusPill variant="neutral" label={t('settings.tabHero.alwaysOn')} />}
    </div>
  )
}
