import type { TranslationDict } from '@mika/sdk/i18n'
import { lazy, memo, type ReactElement, Suspense } from 'react'
import { useI18n } from '../../hooks/useI18n'
import type { useSettingsController } from '../../hooks/useSettingsController'
import { BootHealthBanner } from './BootHealthBanner'
import { PluginFeaturePanel } from './PluginFeaturePanel'
import { ProviderHealthBanner } from './ProviderHealthBanner'

// Built-in tabs load on first visit: a window that opens on "Personagem" no longer parses the logs
// viewer, the homologation board, the backup wizard and eight other panels it never shows.
const AboutPanel = lazy(() => import('./AboutPanel').then((m) => ({ default: m.AboutPanel })))
const AudioDevicesPanel = lazy(() => import('./AudioDevicesPanel').then((m) => ({ default: m.AudioDevicesPanel })))
const BackupPanel = lazy(() => import('./BackupPanel').then((m) => ({ default: m.BackupPanel })))
const HomologationPanel = lazy(() => import('./HomologationPanel').then((m) => ({ default: m.HomologationPanel })))
const LogsPanel = lazy(() => import('./LogsPanel').then((m) => ({ default: m.LogsPanel })))
const PromptsPanel = lazy(() => import('./PromptsPanel').then((m) => ({ default: m.PromptsPanel })))
const ProviderCredentialsPanel = lazy(() =>
  import('./ProviderCredentialsPanel').then((m) => ({ default: m.ProviderCredentialsPanel })),
)
const SettingsModulesDeck = lazy(() =>
  import('./SettingsModulesDeck').then((m) => ({ default: m.SettingsModulesDeck })),
)
const SettingsOverviewDeck = lazy(() =>
  import('./SettingsOverviewDeck').then((m) => ({ default: m.SettingsOverviewDeck })),
)
const SkinPanel = lazy(() => import('./skin/SkinPanel').then((m) => ({ default: m.SkinPanel })))
const ToolsMcpPanel = lazy(() => import('./ToolsMcpPanel').then((m) => ({ default: m.ToolsMcpPanel })))
const UpdatePanel = lazy(() => import('./UpdatePanel').then((m) => ({ default: m.UpdatePanel })))

type Ctrl = ReturnType<typeof useSettingsController>
type T = (key: keyof TranslationDict) => string

function renderBuiltInTab(ctrl: Ctrl): ReactElement | null {
  if (ctrl.activeTabId === 'audio-devices') {
    return <AudioDevicesPanel onSave={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'logs') {
    return <LogsPanel config={ctrl.config} onSave={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'prompts') {
    return <PromptsPanel />
  }

  if (ctrl.activeTabId === 'updates') {
    return <UpdatePanel config={ctrl.config} saveSetting={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'about') {
    return <AboutPanel />
  }

  if (ctrl.activeTabId === 'skin') {
    return <SkinPanel config={ctrl.config} saveSetting={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'credentials') {
    return <ProviderCredentialsPanel config={ctrl.config} onSave={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'backup') {
    return <BackupPanel config={ctrl.config} onSave={ctrl.saveSetting} />
  }

  if (ctrl.activeTabId === 'tools-mcp') {
    return <ToolsMcpPanel config={ctrl.config} onSelectTab={ctrl.setActiveTab} />
  }

  if (ctrl.activeTabId === 'overview') {
    return (
      <SettingsOverviewDeck
        declarations={ctrl.declarations}
        config={ctrl.config}
        health={ctrl.health}
        onSelectTab={ctrl.setActiveTab}
        onSave={ctrl.saveSetting}
      />
    )
  }

  if (ctrl.activeTabId === 'homologation') {
    return <HomologationPanel declarations={ctrl.declarations} onSelectTab={ctrl.setActiveTab} />
  }

  if (ctrl.activeTabId !== 'modules') return null

  return (
    <SettingsModulesDeck
      declarations={ctrl.declarations}
      activeTabId={ctrl.activeTabId}
      config={ctrl.config}
      toolCount={ctrl.health?.tools}
      onSave={ctrl.saveSetting}
      onSelectTab={ctrl.setActiveTab}
    />
  )
}

function emptySelectionState(t: T): ReactElement {
  return (
    <div role="status" className="flex h-60 flex-col items-center justify-center gap-3 opacity-70">
      <div className="h-12 w-12 rounded-full border-2 border-dashed border-border-strong" aria-hidden="true" />
      <p className="text-xs font-bold uppercase tracking-widest text-text-muted">
        {t('shell.settingsPanel.selectSection')}
      </p>
    </div>
  )
}

function renderPluginTab(ctrl: Ctrl): ReactElement | null {
  if (!ctrl.active) return null

  return (
    <PluginFeaturePanel
      active={ctrl.active}
      config={ctrl.config}
      health={ctrl.health}
      settingsPanels={ctrl.activeSettingsPanels}
      onSave={ctrl.saveSetting}
      onBack={() => ctrl.setActiveTab('modules')}
    />
  )
}

const PANEL_CTRL_FIELDS = [
  'activeTabId',
  'declarations',
  'config',
  'health',
  'active',
  'activeSettingsPanels',
  'saveSetting',
  'setActiveTab',
] as const

function panelPropsEqual(prev: { ctrl: Ctrl }, next: { ctrl: Ctrl }): boolean {
  return PANEL_CTRL_FIELDS.every((key) => prev.ctrl[key] === next.ctrl[key])
}

/** Placeholder while a tab's chunk loads (local disk: usually one frame). Same rhythm as the hero + sections. */
function PanelLoading() {
  return (
    <div aria-busy="true" className="space-y-4">
      <div className="h-24 rounded-2xl bg-surface-raise animate-pulse" />
      <div className="h-48 rounded-2xl bg-surface-raise animate-pulse" />
    </div>
  )
}

// The tab transition lives on the page container in SettingsApp — this used to animate a second time
// (x: 6, after the parent's own fade), so every tab switch slid twice.
export const PanelContent = memo(function PanelContent({ ctrl }: { ctrl: Ctrl; accentHue?: number }) {
  const { t } = useI18n()
  const builtInTab = renderBuiltInTab(ctrl)

  return (
    <>
      <ProviderHealthBanner />
      <BootHealthBanner health={ctrl.health} onSelectTab={ctrl.setActiveTab} />
      <Suspense fallback={<PanelLoading />}>
        {builtInTab || ((ctrl.active ? renderPluginTab(ctrl) : emptySelectionState(t)) ?? null)}
      </Suspense>
    </>
  )
}, panelPropsEqual)
