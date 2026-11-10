import { choiceOption } from '@mika/sdk-ui/settings'
import {
  BriefcaseBusiness,
  CircleDashed,
  Coffee,
  Mars,
  Sparkles,
  Target,
  Venus,
  VenusAndMars,
  Wrench,
} from 'lucide-react'
import { configNum, configStr, pick } from '../../../renderer/src/components/settings/settings-utils'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { USER_PROFILE_FILLABLE_KEYS } from '../../../shared/user-profile-fields'

export type Props = {
  name: string
  nickname: string
  birthDate: string
  age: number | undefined
  gender: string
  country: string
  state: string
  city: string
  timezone: string
  profession: string
  company: string
  role: string
  skills: string
  hobbies: string
  favoriteGames: string
  favoriteAnime: string
  favoriteMusic: string
  favoriteFood: string
  partnerName: string
  partnerRelationship: string
  partnerLikes: string
  partnerNotes: string
  personalityTraits: string
  communicationStyle: string
  communicationNotes: string
  bio: string
  notes: string
  learnedByLLM: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export type FieldOpts = {
  placeholder?: string
  type?: 'text' | 'number' | 'password' | 'url' | 'email' | 'textarea'
  rows?: number
}

export function buildGenderOptions(t: Translate) {
  return [
    choiceOption(t('settings.user.field.gender.option.unset'), '', undefined, CircleDashed),
    choiceOption(t('settings.user.field.gender.option.male'), 'masculino', undefined, Mars),
    choiceOption(t('settings.user.field.gender.option.female'), 'feminino', undefined, Venus),
    choiceOption(t('settings.user.field.gender.option.other'), 'outro', undefined, VenusAndMars),
  ]
}

export function buildRelationshipOptions(t: Translate) {
  return [
    { label: t('settings.user.field.partnerRelationship.option.unset'), value: '' },
    { label: t('settings.user.field.partnerRelationship.option.wife'), value: 'esposa' },
    { label: t('settings.user.field.partnerRelationship.option.girlfriend'), value: 'namorada' },
    { label: t('settings.user.field.partnerRelationship.option.fiancee'), value: 'noiva' },
    { label: t('settings.user.field.partnerRelationship.option.companion'), value: 'companheira' },
    { label: t('settings.user.field.partnerRelationship.option.husband'), value: 'marido' },
    { label: t('settings.user.field.partnerRelationship.option.partner'), value: 'parceira' },
  ]
}

export function buildCommunicationOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.user.field.communicationStyle.option.default'),
      '',
      t('settings.user.field.communicationStyle.hint.default'),
      Sparkles,
    ),
    choiceOption(
      t('settings.user.field.communicationStyle.option.direct'),
      'direto',
      t('settings.user.field.communicationStyle.hint.direct'),
      Target,
    ),
    choiceOption(
      t('settings.user.field.communicationStyle.option.casual'),
      'descontraído',
      t('settings.user.field.communicationStyle.hint.casual'),
      Coffee,
    ),
    choiceOption(
      t('settings.user.field.communicationStyle.option.formal'),
      'formal',
      t('settings.user.field.communicationStyle.hint.formal'),
      BriefcaseBusiness,
    ),
    choiceOption(
      t('settings.user.field.communicationStyle.option.technical'),
      'técnico',
      t('settings.user.field.communicationStyle.hint.technical'),
      Wrench,
    ),
  ]
}

function hasValue(value: unknown): boolean {
  if (value === undefined || value === null) return false
  return String(value).trim().length > 0
}

/**
 * O schema declara `age` com mínimo e default 1 (no original era opcional, sem default): 1 é o
 * "não informado" gravado, não uma idade. Mostrado como valor, o campo vinha preenchido com "1" e
 * ainda contava para a completude do perfil.
 */
function realAge(age: number | undefined): number | undefined {
  return age !== undefined && age > 1 ? age : undefined
}

function withRealAge(config: Record<string, unknown>, age: number | undefined): Record<string, unknown> {
  const user = config.user
  if (age !== undefined || !user || typeof user !== 'object') return config
  return { ...config, user: { ...(user as Record<string, unknown>), age: undefined } }
}

export type ProfileCompletion = { filled: number; total: number; percent: number }

function buildConfig(props: Props): Record<string, unknown> {
  const cfg: Record<string, unknown> = {}
  for (const k of USER_PROFILE_FILLABLE_KEYS) {
    cfg[k] = props[k.replace(/^user\./, '') as keyof Props]
  }
  return cfg
}

export type UserProfilePanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function resolveUserProps(props: UserProfilePanelProps): {
  props: Props
  config: Record<string, unknown>
  completion: ProfileCompletion
} {
  const rawCfg = props.config ?? {}
  const resolvedProps: Props = {
    name: pick(props.name, () => configStr(rawCfg, 'user.name')),
    nickname: pick(props.nickname, () => configStr(rawCfg, 'user.nickname')),
    birthDate: pick(props.birthDate, () => configStr(rawCfg, 'user.birthDate')),
    age: realAge(props.age !== undefined ? props.age : configNum(rawCfg, 'user.age')),
    gender: pick(props.gender, () => configStr(rawCfg, 'user.gender')),
    country: pick(props.country, () => configStr(rawCfg, 'user.country')),
    state: pick(props.state, () => configStr(rawCfg, 'user.state')),
    city: pick(props.city, () => configStr(rawCfg, 'user.city')),
    timezone: pick(props.timezone, () => configStr(rawCfg, 'user.timezone')),
    profession: pick(props.profession, () => configStr(rawCfg, 'user.profession')),
    company: pick(props.company, () => configStr(rawCfg, 'user.company')),
    role: pick(props.role, () => configStr(rawCfg, 'user.role')),
    skills: pick(props.skills, () => configStr(rawCfg, 'user.skills')),
    hobbies: pick(props.hobbies, () => configStr(rawCfg, 'user.hobbies')),
    favoriteGames: pick(props.favoriteGames, () => configStr(rawCfg, 'user.favoriteGames')),
    favoriteAnime: pick(props.favoriteAnime, () => configStr(rawCfg, 'user.favoriteAnime')),
    favoriteMusic: pick(props.favoriteMusic, () => configStr(rawCfg, 'user.favoriteMusic')),
    favoriteFood: pick(props.favoriteFood, () => configStr(rawCfg, 'user.favoriteFood')),
    partnerName: pick(props.partnerName, () => configStr(rawCfg, 'user.partnerName')),
    partnerRelationship: pick(props.partnerRelationship, () => configStr(rawCfg, 'user.partnerRelationship')),
    partnerLikes: pick(props.partnerLikes, () => configStr(rawCfg, 'user.partnerLikes')),
    partnerNotes: pick(props.partnerNotes, () => configStr(rawCfg, 'user.partnerNotes')),
    personalityTraits: pick(props.personalityTraits, () => configStr(rawCfg, 'user.personalityTraits')),
    communicationStyle: pick(props.communicationStyle, () => configStr(rawCfg, 'user.communicationStyle')),
    communicationNotes: pick(props.communicationNotes, () => configStr(rawCfg, 'user.communicationNotes')),
    bio: pick(props.bio, () => configStr(rawCfg, 'user.bio')),
    notes: pick(props.notes, () => configStr(rawCfg, 'user.notes')),
    learnedByLLM: pick(props.learnedByLLM, () => configStr(rawCfg, 'user.learnedByLLM')),
    onSave: props.onSave,
  }

  const filled = USER_PROFILE_FILLABLE_KEYS.filter((path) =>
    hasValue(resolvedProps[path.replace(/^user\./, '') as keyof Props]),
  ).length
  const total = USER_PROFILE_FILLABLE_KEYS.length
  const completion: ProfileCompletion = { filled, total, percent: Math.round((filled / total) * 100) }

  const cfg = props.config ? withRealAge(props.config, resolvedProps.age) : buildConfig(resolvedProps)
  return { props: resolvedProps, config: cfg, completion }
}
