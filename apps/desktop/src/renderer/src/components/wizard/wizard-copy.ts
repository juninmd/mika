import type { SupportedLocale } from '@mika/sdk/contracts'
import type { LLMProviderId } from '@mika/sdk/llm/llm-catalog-data'
import { deDE } from './copy/de-de'
import { enUS } from './copy/en-us'
import { esES } from './copy/es-es'
import { frFR } from './copy/fr-fr'
import { itIT } from './copy/it-it'
import { jaJP } from './copy/ja-jp'
import { koKR } from './copy/ko-kr'
import { ptBR } from './copy/pt-br'
import { ruRU } from './copy/ru-ru'
import { zhCN } from './copy/zh-cn'
import type { Step } from './wizard-data'

type TtsOptionId = 'none' | 'supertonic' | 'elevenlabs'

interface WizardOptionCopy {
  name: string
  description: string
}

/** Wizard guidance while the Ollama provider is selected (`OllamaSetupGuide`). */
export type OllamaGuideCopy = {
  offlineTitle: string
  offlineBody: string
  install: string
  retry: string
  noModelsTitle: string
  noModelsBody: string
  pull: string
  pulling: string
  pullDone: string
  pullError: string
  cancel: string
  openError: string
  ready: string
}

export interface WizardCopy {
  badge: string
  stepPrefix: string
  back: string
  // One entry per `WIZARD_STEPS` item, same order.
  steps: readonly [
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
    WizardOptionCopy,
  ]
  /** Her state around the bubble; what she says in it comes from the LLM, never from here. */
  mika: {
    label: string
    asleep: string
    awake: string
    thinking: string
    /** What each step is for — narration shown under her line, never presented as her speech. */
    stepHints: Record<Exclude<Step, 'language'>, string>
  }
  language: {
    step: string
    title: string
    description: string
    continue: string
    saveError: string
  }
  privacy: {
    step: string
    title: string
    description: string
    screenTitle: string
    screenDescription: string
    microphoneTitle: string
    microphoneDescription: string
    privateTitle: string
    privateDescription: string
    saving: string
    continue: string
    saveError: string
  }
  llm: {
    step: string
    title: string
    description: string
    modelLabel: string
    modelHint: string
    modelLoading: string
    saving: string
    continue: string
    skip: string
    skipHint: string
    saveError: string
    credentialRequiredError: string
    providers: Record<LLMProviderId, string>
    ollama: OllamaGuideCopy
  }
  tts: {
    step: string
    title: string
    description: string
    options: Record<TtsOptionId, WizardOptionCopy>
    preview: string
    previewStop: string
    previewLoading: string
    previewHint: string
    back: string
    saving: string
    complete: string
    saveError: string
  }
  /** Escolhe QUEM escuta (`voice.asrProvider`); o ligar a escuta é o toggle do passo de privacidade. */
  stt: {
    step: string
    title: string
    description: string
    back: string
    saving: string
    complete: string
    saveError: string
  }
  embedding: {
    step: string
    title: string
    description: string
    memoryTitle: string
    memoryDescription: string
    saving: string
    continue: string
    saveError: string
  }
  avatar: {
    step: string
    title: string
    description: string
    continue: string
  }
  identity: {
    step: string
    title: string
    description: string
    sectionTitle: string
    sectionDescription: string
    nameLabel: string
    namePlaceholder: string
    nicknameLabel: string
    nicknamePlaceholder: string
    saving: string
    continue: string
    saveError: string
  }
  done: {
    badge: string
    title: string
    description: string
    brainLabel: string
    brainValue: string
    brainDescription: string
    brainPendingValue: string
    brainPendingDescription: string
    voiceLabel: string
    voiceValue: string
    voiceDescription: string
    shortcutsTitle: string
    shortcutsChat: string
    shortcutsMic: string
    shortcutsHistory: string
    shortcutsPushToTalk: string
    micCheckTitle: string
    micCheckStart: string
    micCheckStop: string
    micCheckHint: string
    micCheckHeard: string
    micCheckFailed: string
    enter: string
  }
  assets: {
    back: string
    preparing: string
    readyEyebrow: string
    readyTitle: string
    preparingEyebrow: string
    preparingTitle: string
    statusRunning: string
    statusDone: string
    statusSkipped: string
    statusError: string
    statusPending: string
    errorHint: string
    retry: string
    downloadFailed: string
    vadLabel: string
    vadDetail: string
    sttLabel: string
    sttDetail: string
    ttsLabel: string
    ttsSupertonic: string
    ttsCloud: string
    llmLabel: string
    embeddingLabel: string
    embeddingDetail: string
  }
}

const wizardCopy: Record<SupportedLocale, WizardCopy> = {
  'en-US': enUS,
  'pt-BR': ptBR,
  'es-ES': esES,
  'fr-FR': frFR,
  'it-IT': itIT,
  'ja-JP': jaJP,
  'zh-CN': zhCN,
  'ko-KR': koKR,
  'ru-RU': ruRU,
  'de-DE': deDE,
}

export function getWizardCopy(locale: SupportedLocale): WizardCopy {
  return wizardCopy[locale]
}
