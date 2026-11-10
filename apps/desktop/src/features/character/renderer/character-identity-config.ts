import { getLocale, LOCALE_OPTIONS } from '@mika/sdk/i18n'
import { choiceOption } from '@mika/sdk-ui/settings'
import {
  Angry,
  BookOpen,
  Briefcase,
  Contrast,
  Drama,
  Ghost,
  type LucideIcon,
  Scale,
  Smile,
  Sparkles,
  Square,
} from 'lucide-react'
import type { Relationship as RelationshipStatus } from '../../../renderer/src/components/settings/RelationshipSelector'
import { configBool, configNum, configStr, pick } from '../../../renderer/src/components/settings/settings-utils'
import type { Translate } from '../../../renderer/src/hooks/useI18n'

type Props = {
  enabled: boolean
  name: string
  personality: string
  personalityProfile: string
  visualDescription: string
  greetingStyle: string
  responseStyle: string
  modelName: string
  modelPath: string
  backgroundMode: string
  backgroundColor: string
  hobbies: string
  dislikes: string
  nicknamesForUser: string
  secretGoals: string
  customInstructions: string
  relationshipStatus?: RelationshipStatus
  affinityScore?: number
  appLocale?: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export const APP_LOCALE_OPTIONS = LOCALE_OPTIONS.map((opt) => ({ label: opt.nativeName, value: opt.locale }))

export function responseStyleOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.responseStyle.option.playful'),
      'playful',
      t('settings.character.field.responseStyle.hint.playful'),
      Sparkles,
    ),
    choiceOption(
      t('settings.character.field.responseStyle.option.balanced'),
      'balanced',
      t('settings.character.field.responseStyle.hint.balanced'),
      Scale,
    ),
    choiceOption(
      t('settings.character.field.responseStyle.option.formal'),
      'formal',
      t('settings.character.field.responseStyle.hint.formal'),
      BookOpen,
    ),
  ]
}

/** Os rótulos de perfil já trazem "Nome — traços" em todo locale; o card mostra os traços como descrição. */
function profileChoice(text: string, value: string, icon: LucideIcon) {
  const [name = text, ...traits] = text.split(/\s*—+\s*/)
  const description = traits.join(' — ')
  return choiceOption(name, value, description.charAt(0).toLocaleUpperCase() + description.slice(1) || undefined, icon)
}

export function profileOptions(t: Translate) {
  return [
    profileChoice(t('settings.character.identityPanel.option.friendDefault'), 'friend', Smile),
    profileChoice(t('settings.character.field.personalityProfile.option.professional'), 'professional', Briefcase),
    profileChoice(t('settings.character.field.personalityProfile.option.tsundere'), 'tsundere', Angry),
    profileChoice(t('settings.character.field.personalityProfile.option.sarcastic'), 'sarcastic', Drama),
  ]
}

export function backgroundModeOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.identityPanel.option.bgTransparentOverlay'),
      'transparent',
      t('settings.character.field.backgroundMode.hint.transparent'),
      Ghost,
    ),
    choiceOption(
      t('settings.character.field.backgroundMode.option.semiTransparent'),
      'semi-transparent',
      t('settings.character.field.backgroundMode.hint.semiTransparent'),
      Contrast,
    ),
    choiceOption(
      t('settings.character.field.backgroundMode.option.solid'),
      'solid',
      t('settings.character.field.backgroundMode.hint.solid'),
      Square,
    ),
  ]
}

export type CharacterIdentityPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function resolveIdentityConfig(props: CharacterIdentityPanelProps) {
  const rawCfg = props.config ?? {}
  const enabled = pick(props.enabled, () => configBool(rawCfg, 'character.enabled', true))
  const name = pick(props.name, () => configStr(rawCfg, 'character.name', 'Mika'))
  const personality = pick(props.personality, () => configStr(rawCfg, 'character.personality', ''))
  const personalityProfile = pick(props.personalityProfile, () =>
    configStr(rawCfg, 'character.personalityProfile', 'friend'),
  )
  const appLocale = pick(props.appLocale, () => configStr(rawCfg, 'app.locale', getLocale()))
  const visualDescription = pick(props.visualDescription, () => configStr(rawCfg, 'character.visualDescription', ''))
  const greetingStyle = pick(props.greetingStyle, () => configStr(rawCfg, 'character.greetingStyle', ''))
  const responseStyle = pick(props.responseStyle, () => configStr(rawCfg, 'character.responseStyle', 'playful'))
  const modelName = pick(props.modelName, () => configStr(rawCfg, 'character.modelName', ''))
  const modelPath = pick(props.modelPath, () => configStr(rawCfg, 'character.modelPath', ''))
  const backgroundMode = pick(props.backgroundMode, () => configStr(rawCfg, 'character.backgroundMode', 'transparent'))
  const backgroundColor = pick(props.backgroundColor, () =>
    configStr(rawCfg, 'character.backgroundSolidColor', '#050914'),
  )
  const hobbies = pick(props.hobbies, () => configStr(rawCfg, 'character.hobbies', ''))
  const dislikes = pick(props.dislikes, () => configStr(rawCfg, 'character.dislikes', ''))
  const nicknamesForUser = pick(props.nicknamesForUser, () => configStr(rawCfg, 'character.nicknamesForUser', ''))
  const secretGoals = pick(props.secretGoals, () => configStr(rawCfg, 'character.secretGoals', ''))
  const customInstructions = pick(props.customInstructions, () => configStr(rawCfg, 'character.customInstructions', ''))
  const relationshipStatus = pick(props.relationshipStatus, () =>
    toRelationship(configStr(rawCfg, 'character.relationshipStatus', 'friend')),
  )
  const affinityScore = pick(props.affinityScore, () => configNum(rawCfg, 'character.affinityScore', 0))

  const cfg: Record<string, unknown> = props.config ?? {
    'character.enabled': enabled,
    'character.name': name,
    'character.personality': personality,
    'character.personalityProfile': personalityProfile,
    'app.locale': appLocale,
    'character.visualDescription': visualDescription,
    'character.greetingStyle': greetingStyle,
    'character.responseStyle': responseStyle,
    'character.modelName': modelName,
    'character.modelPath': modelPath,
    'character.backgroundMode': backgroundMode,
    'character.backgroundSolidColor': backgroundColor,
    'character.hobbies': hobbies,
    'character.dislikes': dislikes,
    'character.nicknamesForUser': nicknamesForUser,
    'character.secretGoals': secretGoals,
    'character.customInstructions': customInstructions,
  }

  return { enabled, name, modelName, modelPath, backgroundMode, relationshipStatus, affinityScore, cfg }
}

const RELATIONSHIPS: readonly RelationshipStatus[] = ['acquaintance', 'friend', 'close-friend', 'companion']

function toRelationship(raw: string): RelationshipStatus {
  return RELATIONSHIPS.find((r) => r === raw) ?? 'friend'
}
