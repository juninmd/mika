export interface PluginUiBaseDeclaration {
  id: string
  pluginId: string
  title: string
  component: string
  description?: string
}

export interface PluginViewDeclaration extends PluginUiBaseDeclaration {
  surface?: 'view'
  defaultWindow?: {
    width: number
    height: number
    minWidth?: number
    minHeight?: number
  }
}

export type SettingsPanelPlacement = 'before-schema' | 'after-schema' | 'replace-schema'

export interface SettingsPanelDeclaration extends PluginUiBaseDeclaration {
  surface?: 'settingsPanel'
  placement: SettingsPanelPlacement
}

export interface PluginUiCatalog {
  views: PluginViewDeclaration[]
  settingsPanels: SettingsPanelDeclaration[]
}
