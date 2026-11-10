import type { FeatureSettingsDeclaration, SettingsPanelDeclaration } from '@mika/sdk/contracts'
import { dedupeBy } from '@mika/sdk/utils/dedupe'
import type { ReactElement } from 'react'
import { rendererLog } from '../../lib/renderer-logger'
import { resolveFeatureComponent } from './feature-components'
import { PluginSettingsPanels } from './PluginSettingsPanels'
import { PluginToolsPanel } from './PluginToolsPanel'
import { SettingsGroupPanel } from './SettingsGroupPanel'
import { SettingsTabHero } from './SettingsTabHero'

type PluginFeaturePanelProps = {
  active: FeatureSettingsDeclaration
  config: Record<string, unknown>
  health: { status: string } | null
  settingsPanels: SettingsPanelDeclaration[]
  onSave: (path: string, value: unknown) => Promise<void>
  onBack: () => void
}

function dedupeComponentGroups(groups: FeatureSettingsDeclaration['groups']): FeatureSettingsDeclaration['groups'] {
  let noComponentIndex = 0
  return dedupeBy(groups, (group) => group.component ?? `__no-component-${noComponentIndex++}`)
}

function panelsByPlacement(
  panels: SettingsPanelDeclaration[],
  placement: SettingsPanelDeclaration['placement'],
): SettingsPanelDeclaration[] {
  return panels.filter((panel) => panel.placement === placement)
}

export function PluginFeaturePanel({
  active,
  config,
  health,
  settingsPanels,
  onSave,
  onBack,
}: PluginFeaturePanelProps): ReactElement {
  const beforeSchemaPanels = panelsByPlacement(settingsPanels, 'before-schema')
  const replaceSchemaPanels = panelsByPlacement(settingsPanels, 'replace-schema')
  const afterSchemaPanels = panelsByPlacement(settingsPanels, 'after-schema')
  const TabSettingsComponent = resolveFeatureComponent(active.component)

  if (active.component && !TabSettingsComponent) {
    rendererLog.error('[Settings] Tab settings component missing', {
      featureId: active.tabId,
      componentName: active.component,
    })
  }

  const schemaGroups =
    replaceSchemaPanels.length > 0 || TabSettingsComponent ? [] : dedupeComponentGroups(active.groups ?? [])

  for (const group of schemaGroups) {
    if (!group.component) continue
    rendererLog.info('[Settings] Rendering manual settings panel', {
      featureId: active.tabId,
      groupId: group.groupPrefix ?? group.title,
      componentName: group.component,
    })
  }

  return (
    <div className="space-y-4">
      <SettingsTabHero active={active} config={config} health={health} onSave={onSave} onBack={onBack} />
      {active.toolDetails && active.toolDetails.length > 0 && (
        <PluginToolsPanel tools={active.toolDetails} pluginId={active.pluginId} />
      )}
      {beforeSchemaPanels.length > 0 && <PluginSettingsPanels panels={beforeSchemaPanels} />}
      {TabSettingsComponent && (
        <TabSettingsComponent config={config} onSave={onSave} tabId={active.tabId} pluginId={active.pluginId} />
      )}
      {schemaGroups.map((group) => (
        <SettingsGroupPanel
          key={`${group.groupPrefix ?? group.title}:${group.component ?? 'schema'}`}
          config={config}
          group={group}
          tabId={active.tabId}
          pluginId={active.pluginId}
          onSave={onSave}
        />
      ))}
      {replaceSchemaPanels.length > 0 && <PluginSettingsPanels panels={replaceSchemaPanels} />}
      {afterSchemaPanels.length > 0 && <PluginSettingsPanels panels={afterSchemaPanels} />}
    </div>
  )
}
