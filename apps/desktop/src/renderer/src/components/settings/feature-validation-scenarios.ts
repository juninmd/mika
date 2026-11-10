import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import type { TranslateParams, TranslationDict } from '@mika/sdk/i18n'

type FeatureValidationScenario = {
  descriptionKey: keyof TranslationDict
  /** What is being validated, as a noun phrase: the running/success/failure sentences wrap it. */
  subjectKey: keyof TranslationDict
  prompt?: string
  groupPrefix?: string
}

type Translate = (key: keyof TranslationDict, params?: TranslateParams) => string

type FeatureValidationPayload = {
  tabId?: string
  pluginId?: string
  groupPrefix?: string
  prompt?: string
}

const DEFAULT_SCENARIO: FeatureValidationScenario = {
  descriptionKey: 'settings.featureValidation.scenario.default.description',
  subjectKey: 'settings.featureValidation.subject.default',
}

const SCENARIOS_BY_TAB: Record<string, FeatureValidationScenario> = {
  llm: {
    descriptionKey: 'settings.featureValidation.scenario.llm.description',
    subjectKey: 'settings.featureValidation.subject.llm',
    prompt: 'Responda em uma frase curta: Mika LLM operacional.',
  },
  voice: {
    descriptionKey: 'settings.featureValidation.scenario.voice.description',
    subjectKey: 'settings.featureValidation.subject.voice',
  },
  stt: {
    descriptionKey: 'settings.featureValidation.scenario.stt.description',
    subjectKey: 'settings.featureValidation.subject.stt',
  },
  vision: {
    descriptionKey: 'settings.featureValidation.scenario.vision.description',
    subjectKey: 'settings.featureValidation.subject.vision',
  },
  brain: {
    descriptionKey: 'settings.featureValidation.scenario.brain.description',
    subjectKey: 'settings.featureValidation.subject.brain',
  },
  telegram: {
    descriptionKey: 'settings.featureValidation.scenario.telegram.description',
    subjectKey: 'settings.featureValidation.subject.telegram',
  },
  'person-catalog': {
    descriptionKey: 'settings.featureValidation.scenario.personCatalog.description',
    subjectKey: 'settings.featureValidation.subject.personCatalog',
  },
  'gamer-companion': {
    descriptionKey: 'settings.featureValidation.scenario.gamerCompanion.description',
    subjectKey: 'settings.featureValidation.subject.gamerCompanion',
  },
}

export function getFeatureValidationScenario(feature: FeatureSettingsDeclaration): FeatureValidationScenario {
  return SCENARIOS_BY_TAB[feature.tabId] ?? DEFAULT_SCENARIO
}

export function buildFeatureValidationPayload(feature: FeatureSettingsDeclaration): FeatureValidationPayload {
  const scenario = getFeatureValidationScenario(feature)
  return {
    tabId: feature.tabId,
    pluginId: feature.pluginId ?? feature.tabId,
    ...(scenario.groupPrefix ? { groupPrefix: scenario.groupPrefix } : {}),
    ...(scenario.prompt ? { prompt: scenario.prompt } : {}),
  }
}

/** Running/success/failure lines for a scenario, in the language `t` speaks (the backend may override success/failure). */
export function validationMessages(scenario: FeatureValidationScenario, t: Translate) {
  const name = t(scenario.subjectKey)
  return {
    running: t('settings.featureValidation.running', { name }),
    success: t('settings.featureValidation.success', { name }),
    failure: t('settings.featureValidation.failure', { name }),
  }
}
