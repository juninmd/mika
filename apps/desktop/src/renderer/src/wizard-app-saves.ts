import { ConfigKey } from '@mika/sdk/contracts'
import type { PrivacyChoices } from './components/wizard/PrivacyStep'
import type { ProviderId, Step } from './components/wizard/wizard-data'
import { LLM_PROVIDERS } from './components/wizard/wizard-data'
import type { SttSelection } from './components/wizard/wizard-stt'
import { readSavedStt, sttSettingsToSave } from './components/wizard/wizard-stt'
import { readSavedTts, type TtsSelection, ttsSettingsToSave } from './components/wizard/wizard-tts'
import { saveLegalAcceptance } from './lib/legal-acceptance'
import { getElectronApi } from './lib/shell-api'

export const runtimeApi = () => getElectronApi().runtime

// Também a ordem de navegação: `use-wizard-state.ts` deriva `activeStepIndex` do índice aqui —
// um único lugar para a sequência do onboarding, em vez de duas listas que podem divergir.
// Ordem dos sentidos: ela ganha o corpo (skin) antes do cérebro, para a primeira fala já comentar o
// próprio visual; cada sentido seguinte chega com o cérebro ligado e ela reage a ele.
// Escuta na sequência da voz: os sentidos de áudio (quem fala, quem ouve) chegam juntos.
export const WIZARD_STEPS: Step[] = [
  'language',
  'avatar',
  'llm',
  'tts',
  'stt',
  'embedding',
  'privacy',
  'identity',
  'done',
]

// Mantém o wizard alinhado ao resolvedor do Main: cada provedor tem sua própria chave
// persistida, e `llm.model` serve apenas como fallback legado.
const MODEL_KEY_FOR_PROVIDER: Partial<Record<ProviderId, ConfigKey>> = {
  google: ConfigKey.LlmGoogleModel,
  openrouter: ConfigKey.LlmOpenrouterModel,
  ollama: ConfigKey.LlmOllamaModel,
  openaiCompatible: ConfigKey.LlmOpenaiCompatibleModel,
  unsloth: ConfigKey.LlmUnslothModel,
}

function isLlmConfiguredInSavedConfig(config: Record<string, unknown>): boolean {
  const llm = config.llm as Record<string, unknown> | undefined
  const savedProvider = llm?.provider
  const configuredProvider =
    typeof savedProvider === 'string' ? LLM_PROVIDERS.find((item) => item.id === savedProvider) : undefined
  if (!configuredProvider) return false

  const credentialKey = configuredProvider.credentialConfigKey
  if (!credentialKey || configuredProvider.id === 'ollama') return true

  const apiKeys = config.apiKeys as Record<string, unknown> | undefined
  return Boolean(apiKeys?.[credentialKey.split('.').pop() ?? ''])
}

// Restaura os toggles de mic/tela do que já está salvo — sem isto, reabrir o wizard num app já
// configurado ("refazer o setup" da paleta) mostrava os dois APAGADOS mesmo com STT/vision ligados,
// e um clique em "Continuar" gravava `false` por cima do que o usuário já tinha concedido.
function readSavedPrivacy(config: Record<string, unknown>): PrivacyChoices {
  const voice = config.voice as Record<string, unknown> | undefined
  const privacy = config.privacy as Record<string, unknown> | undefined
  return {
    microphone: voice?.sttEnabled === true,
    screen: privacy?.screenCapture === true,
  }
}

export function readWizardRestore(config: Record<string, unknown>) {
  const setup = config.setup as Record<string, unknown> | undefined
  const savedStep = setup?.wizardCurrentStep
  const llmConfigured = isLlmConfiguredInSavedConfig(config as Record<string, unknown>)
  // `privacy` without a brain is the OLD order's 2nd step: resuming it in the new order would skip
  // skin, brain, voice and memory, so it starts over.
  const legacyPrivacy = savedStep === 'privacy' && !llmConfigured
  return {
    step:
      !legacyPrivacy && typeof savedStep === 'string' && WIZARD_STEPS.includes(savedStep as Step)
        ? (savedStep as Step)
        : undefined,
    tts: readSavedTts(config as Record<string, unknown>),
    stt: readSavedStt(config as Record<string, unknown>),
    privacy: readSavedPrivacy(config as Record<string, unknown>),
    memory: (config.memory as Record<string, unknown> | undefined)?.enabled === true,
    llmConfigured,
  }
}

// `hadExistingSetup` distingue instalação nova de "refazer o setup": só no primeiro caso os
// defaults conservadores de memória/clipboard fazem sentido — reaplicá-los numa sessão que já
// tinha um LLM configurado apagaria em silêncio o que o usuário já tinha ligado nas Configurações
// (mesma classe do bug que zerava `voice.ttsEnabled`, ver `readSavedTts`).
export function savePrivacyChoices(privacyChoices: PrivacyChoices, hadExistingSetup: boolean) {
  const saves = [
    runtimeApi().saveSetting(ConfigKey.PrivacyScreenCapture, privacyChoices.screen),
    runtimeApi().saveSetting(ConfigKey.VisionEnabled, privacyChoices.screen),
    runtimeApi().saveSetting(ConfigKey.VoiceSttEnabled, privacyChoices.microphone),
  ]
  if (!hadExistingSetup) {
    saves.push(
      runtimeApi().saveSetting(ConfigKey.PrivacyClipboardLogging, false),
      runtimeApi().saveSetting('vision.readClipboardImages', false),
      runtimeApi().saveSetting('vision.visualDiaryEnabled', false),
    )
  }
  return Promise.all(saves)
}

// O aceite do EULA/política fecha junto com o passo de privacidade: um "Continuar" só, e uma falha
// em qualquer gravação aparece no mesmo erro de `runWizardSave`.
export async function savePrivacyStep(privacyChoices: PrivacyChoices, hadExistingSetup: boolean, legalVersion: string) {
  const results = await Promise.all([
    savePrivacyChoices(privacyChoices, hadExistingSetup),
    saveLegalAcceptance(legalVersion),
  ])
  return results.flat()
}

// A memória é decidida no passo de embedding, que agora vem ANTES da privacidade: se o default
// conservador (`false`) continuasse em `savePrivacyChoices`, apagaria a escolha feita um passo antes.
// Numa instalação nova ele é gravado já no idioma (1º passo): assim, retomar o wizard depois do LLM
// lê `false` da config, e não o default do schema (`true`) que ninguém escolheu.
export function saveFreshInstallMemoryDefault() {
  return runtimeApi().saveSetting('memory.enabled', false)
}

// Mesma classe da memória: `voice.sttEnabled` tem default `true` no schema e a config volta com os
// defaults mesclados, então o passo de privacidade ("ficam desligados até você ativar") abriria com o
// microfone ligado numa instalação nova. O `false` explícito do 1º passo é o que o toggle lê de volta.
export function saveFreshInstallMicDefault() {
  return runtimeApi().saveSetting(ConfigKey.VoiceSttEnabled, false)
}

export function saveMemoryChoice(enabled: boolean) {
  return Promise.all([runtimeApi().saveSetting('memory.enabled', enabled)])
}

export function saveLlmSelection(
  provider: ProviderId,
  model: string,
  apiKey: string,
  credentialConfigKey: string | undefined,
) {
  const modelKey = MODEL_KEY_FOR_PROVIDER[provider]
  const saves: Array<Promise<{ success: boolean }>> = [
    runtimeApi().saveSetting(ConfigKey.LlmProvider, provider),
    runtimeApi().saveSetting(ConfigKey.LlmModel, model),
  ]
  if (modelKey) saves.push(runtimeApi().saveSetting(modelKey, model))

  if (apiKey.trim() && credentialConfigKey) {
    saves.push(runtimeApi().saveSetting(credentialConfigKey, apiKey.trim()))
  }
  return Promise.all(saves)
}

export function saveTtsCompletion(tts: TtsSelection) {
  return Promise.all(ttsSettingsToSave(tts).map(([key, value]) => runtimeApi().saveSetting(key, value)))
}

// Escolha de escuta (QUEM ouve): só grava o provedor — o toggle de escuta em si (sttEnabled) é do
// passo de privacidade (microfone), guardado por `savePrivacyChoices`. null = manter o que há.
export function saveSttCompletion(stt: SttSelection) {
  if (stt === null) return Promise.all([])
  return Promise.all(sttSettingsToSave(stt).map(([key, value]) => runtimeApi().saveSetting(key, value)))
}

// Fecha o onboarding — chamado depois do ÚLTIMO passo substantivo (Identidade), não mais logo
// após a voz: Avatar/Identidade ficariam sem serem percorridos e `wizardCompleted` já teria virado
// `true`. Reseta `wizardCurrentStep` para `language` para uma reabertura futura ("refazer o setup")
// começar do início, e não retomar no meio de um fluxo que já terminou.
export function finishWizardSetup() {
  return Promise.all([
    runtimeApi().saveSetting(ConfigKey.SetupWizardCompleted, true),
    runtimeApi().saveSetting(ConfigKey.SetupWizardCurrentStep, 'language'),
  ])
}

export async function runWizardSave(
  save: () => Promise<Array<{ success: boolean }>>,
  saveError: string,
  onSuccess: () => void,
  setError: (error: string) => void,
  setSaving: (saving: boolean) => void,
) {
  try {
    const results = await save()
    if (results.some((result) => !result.success)) {
      setError(saveError)
      return
    }
    onSuccess()
  } catch {
    setError(saveError)
  } finally {
    setSaving(false)
  }
}
