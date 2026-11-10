export type SettingsFieldType =
  | 'text'
  | 'number'
  | 'range'
  | 'boolean'
  | 'select'
  | 'password'
  | 'textarea'
  | 'url'
  | 'object'

export interface ToolParameterDetail {
  name: string
  type: string
  description: string
  required: boolean
}

export interface ToolDetail {
  name: string
  description: string
  inputSchema: ToolParameterDetail[]
}

export type SettingsComparableValue = string | number | boolean | null

export interface SettingsVisibilityRule {
  configPath: string
  equals?: SettingsComparableValue
  notEquals?: SettingsComparableValue
  truthy?: boolean
}

export interface SettingsFieldOption {
  label: string
  value: string
}

export interface SettingsField {
  configPath: string
  type: SettingsFieldType
  label: string
  description?: string
  helperText?: string
  placeholder?: string
  example?: string
  examples?: string[]
  options?: SettingsFieldOption[]
  min?: number
  max?: number
  step?: number
  minLength?: number
  maxLength?: number
  pattern?: string
  patternError?: string
  rows?: number
  unit?: string
  badge?: string
  inputMode?: 'text' | 'numeric' | 'decimal' | 'search' | 'email' | 'url'
  learnMoreUrl?: string
  visibleWhen?: SettingsVisibilityRule
}

export interface SettingsGroup {
  title: string
  description?: string
  badge?: string
  columns?: 1 | 2 | 3
  fields: SettingsField[]
  visibleWhen?: SettingsVisibilityRule
  testAction?: { label?: string }
  groupPrefix?: string

  component?: string
}

export interface FeatureSettingsDeclaration {
  tabId: string
  tabLabel: string
  tabDescription?: string
  tabIcon?: string
  tabGroup?: string

  component?: string
  isCore?: boolean
  pluginId?: string

  enabledConfigPath?: string
  heroNote?: string
  accentColor?: string
  groups: SettingsGroup[]
  prefix?: string
  category?: string
  toolCount?: number
  toolDetails?: ToolDetail[]
}
