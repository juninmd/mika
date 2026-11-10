import { ConfigKey } from '@mika/sdk/contracts'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useModelCatalog } from './components/settings/model-catalog'
import type { PrivacyChoices } from './components/wizard/PrivacyStep'
import { reactionFor, useMikaReaction } from './components/wizard/use-mika-reaction'
import { getWizardCopy } from './components/wizard/wizard-copy'
import type { ProviderId, Step } from './components/wizard/wizard-data'
import {
  DEFAULT_WIZARD_PROVIDER_ID,
  getWizardProvider,
  reconcileModel,
  wizardFallbackModels,
} from './components/wizard/wizard-data'
import type { SttSelection } from './components/wizard/wizard-stt'
import type { TtsSelection } from './components/wizard/wizard-tts'
import { useI18n } from './hooks/useI18n'
import { useLegalDocument } from './hooks/useLegalDocument'
import { isLegalAccepted } from './lib/legal-acceptance'
import { useWizardConfig } from './use-wizard-config'
import {
  finishWizardSetup,
  readWizardRestore,
  runtimeApi,
  runWizardSave,
  saveFreshInstallMemoryDefault,
  saveFreshInstallMicDefault,
  saveLlmSelection,
  saveMemoryChoice,
  savePrivacyStep,
  saveSttCompletion,
  saveTtsCompletion,
  WIZARD_STEPS,
} from './wizard-app-saves'

export function useWizardState() {
  const { locale, setLocale, t } = useI18n()
  const [step, setStep] = useState<Step>('language')
  const [stepRestored, setStepRestored] = useState(false)
  const [provider, setProvider] = useState<ProviderId>(DEFAULT_WIZARD_PROVIDER_ID)
  const [model, setModel] = useState(getWizardProvider(DEFAULT_WIZARD_PROVIDER_ID).modelDefault)
  const [apiKey, setApiKey] = useState('')
  const [llmConfigured, setLlmConfigured] = useState(false)
  const [privacyChoices, setPrivacyChoices] = useState<PrivacyChoices>({ microphone: false, screen: false })
  const [tts, setTts] = useState<TtsSelection>(null)
  const [stt, setStt] = useState<SttSelection>(null)
  // Aceite do EULA/política (checkbox do passo de privacidade). Já vem marcado quando a config
  // guarda o aceite da versão ATUAL — ao retomar o wizard o usuário não aceita duas vezes.
  const [legalAttempt, setLegalAttempt] = useState(0)
  const retryLegal = useCallback(() => setLegalAttempt((n) => n + 1), [])
  const legalDoc = useLegalDocument('eula', legalAttempt)
  const legalVersion = legalDoc.status === 'ready' ? legalDoc.version : ''
  const [legalAccepted, setLegalAccepted] = useState(false)
  const [ageConfirmed, setAgeConfirmed] = useState(false)
  const [memoryEnabled, setMemoryEnabled] = useState(false)
  const { reaction, react } = useMikaReaction()
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  // Sinaliza um "refazer o setup" (command palette) em vez de instalação nova: só nesse caso a
  // config já tinha um LLM configurado ANTES desta sessão abrir. Sem isto, o passo de privacidade
  // reaplica os defaults conservadores do primeiro uso e apaga memória/clipboard de quem já tinha
  // ligado — a mesma classe do bug que zerava `voice.ttsEnabled` num usuário já configurado.
  const [hadExistingSetup, setHadExistingSetup] = useState(false)
  const { config, setConfig, saveConfig } = useWizardConfig()
  const copy = useMemo(() => getWizardCopy(locale), [locale])

  useEffect(() => {
    if (isLegalAccepted(config, legalVersion)) {
      setLegalAccepted(true)
      setAgeConfirmed(true)
    }
  }, [config, legalVersion])

  const selectedProvider = useMemo(() => getWizardProvider(provider), [provider])
  const fallbackModels = useMemo(() => wizardFallbackModels(selectedProvider), [selectedProvider])
  // O Unsloth só lista os modelos carregados com a chave, e o wizard ainda não a gravou: ela vai no
  // payload da consulta, com debounce para sair uma consulta por pausa de digitação, não por tecla.
  const [catalogKey, setCatalogKey] = useState('')
  useEffect(() => {
    if (provider !== 'unsloth') return setCatalogKey('')
    const timer = setTimeout(() => setCatalogKey(apiKey.trim()), 600)
    return () => clearTimeout(timer)
  }, [provider, apiKey])
  const {
    models: modelCatalog,
    loading: modelsLoading,
    reload: reloadModels,
  } = useModelCatalog(provider, 'chat', fallbackModels, catalogKey || undefined)
  const activeStepIndex = useMemo(() => {
    const index = WIZARD_STEPS.indexOf(step)
    return index === -1 ? 0 : index
  }, [step])

  useEffect(() => {
    setModel(selectedProvider.modelDefault)
  }, [selectedProvider.modelDefault])

  // O modelo escolhido sempre existe na lista visível: padrão não instalado (Ollama) ou provedor sem
  // modelo de fábrica (Unsloth) caem no primeiro da lista viva.
  useEffect(() => {
    const next = reconcileModel(model, modelCatalog, modelsLoading)
    if (next !== model) setModel(next)
  }, [model, modelCatalog, modelsLoading])

  useEffect(() => {
    let cancelled = false
    runtimeApi()
      .getSettingsConfig()
      .then(({ config: loadedConfig }) => {
        if (cancelled) return
        const restored = readWizardRestore(loadedConfig)
        if (restored.step) setStep(restored.step)
        setTts(restored.tts)
        setStt(restored.stt)
        setPrivacyChoices(restored.privacy)
        setMemoryEnabled(restored.memory)
        if (restored.llmConfigured) {
          setLlmConfigured(true)
          setHadExistingSetup(true)
        }
        setConfig(loadedConfig)
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setStepRestored(true)
      })
    return () => {
      cancelled = true
    }
  }, [setConfig])

  // Moves on and lets her react to the sense this step just gave her (no brain, no reaction).
  const advance = useCallback(
    (from: Step, to: Step, brain: boolean) => {
      setStep(to)
      const sense = reactionFor(from, brain)
      if (sense) react(sense)
    },
    [react],
  )

  useEffect(() => {
    if (!stepRestored || step === 'done') return
    runtimeApi()
      .saveSetting(ConfigKey.SetupWizardCurrentStep, step)
      .then((result) => {
        if (!result.success) setError(copy.tts.saveError)
      })
      .catch(() => setError(copy.tts.saveError))
  }, [step, stepRestored, copy.tts.saveError])

  const handleLocaleChange = useCallback(
    (nextLocale: typeof locale) => {
      setError('')
      void setLocale(nextLocale).then((ok) => {
        if (!ok) setError(copy.language.saveError)
      })
    },
    [setLocale, copy.language.saveError],
  )

  const handleFinishLanguage = useCallback(async () => {
    setError('')
    const ok = await setLocale(locale)
    // A rejected save must surface as the step's error banner, not as an unhandled rejection on "Continuar".
    const defaultsOff =
      hadExistingSetup ||
      (await Promise.all([saveFreshInstallMemoryDefault(), saveFreshInstallMicDefault()]).then(
        (results) => results.every((result) => result.success),
        () => false,
      ))
    if (!ok || !defaultsOff) {
      setError(copy.language.saveError)
      return
    }
    if (!hadExistingSetup) {
      setMemoryEnabled(false)
      setPrivacyChoices((current) => ({ ...current, microphone: false }))
    }
    setStep('avatar')
  }, [locale, setLocale, copy.language.saveError, hadExistingSetup])

  const handleTogglePrivacy = useCallback((choice: keyof PrivacyChoices) => {
    setPrivacyChoices((current) => ({ ...current, [choice]: !current[choice] }))
  }, [])

  // Trocar de provedor TEM de limpar a chave digitada: antes, uma chave colada para a OpenAI era
  // salva sob a chave de credencial do provedor seguinte, e o erro só aparecia no primeiro request.
  const handleProviderChange = useCallback((next: ProviderId) => {
    setProvider(next)
    setApiKey('')
  }, [])

  const handleFinishPrivacy = useCallback(async () => {
    setError('')
    if (!legalAccepted || !ageConfirmed || !legalVersion) {
      setError(t('legal.consent.required'))
      return
    }
    setSaving(true)

    await runWizardSave(
      () => savePrivacyStep(privacyChoices, hadExistingSetup, legalVersion),
      copy.privacy.saveError,
      () => advance('privacy', 'identity', llmConfigured),
      setError,
      setSaving,
    )
  }, [
    privacyChoices,
    hadExistingSetup,
    legalAccepted,
    ageConfirmed,
    legalVersion,
    t,
    copy.privacy.saveError,
    advance,
    llmConfigured,
  ])

  const handleFinishLlm = useCallback(async () => {
    setError('')

    if (selectedProvider.credentialConfigKey && selectedProvider.id !== 'ollama' && !apiKey.trim()) {
      setError(copy.llm.credentialRequiredError)
      return
    }

    setSaving(true)

    await runWizardSave(
      () => saveLlmSelection(provider, model, apiKey, selectedProvider.credentialConfigKey),
      copy.llm.saveError,
      () => {
        const brain =
          selectedProvider.id === 'ollama' || !selectedProvider.credentialConfigKey || apiKey.trim().length > 0
        setLlmConfigured(brain)
        advance('llm', 'tts', brain)
      },
      setError,
      setSaving,
    )
  }, [
    provider,
    model,
    apiKey,
    selectedProvider.credentialConfigKey,
    selectedProvider.id,
    copy.llm.saveError,
    copy.llm.credentialRequiredError,
    advance,
  ])

  const handleSkipLlm = useCallback(() => {
    setError('')
    setLlmConfigured(false)
    setStep('tts')
  }, [])

  const handleFinishTts = useCallback(async () => {
    setSaving(true)
    setError('')

    await runWizardSave(
      () => saveTtsCompletion(tts),
      copy.tts.saveError,
      () => advance('tts', 'stt', llmConfigured),
      setError,
      setSaving,
    )
  }, [tts, copy.tts.saveError, advance, llmConfigured])

  const handleFinishStt = useCallback(async () => {
    setSaving(true)
    setError('')

    await runWizardSave(
      () => saveSttCompletion(stt),
      copy.stt.saveError,
      () => advance('stt', 'embedding', llmConfigured),
      setError,
      setSaving,
    )
  }, [stt, copy.stt.saveError, advance, llmConfigured])

  const handleFinishEmbedding = useCallback(async () => {
    setSaving(true)
    setError('')
    await runWizardSave(
      () => saveMemoryChoice(memoryEnabled),
      copy.embedding.saveError,
      () => advance('embedding', 'privacy', llmConfigured),
      setError,
      setSaving,
    )
  }, [memoryEnabled, copy.embedding.saveError, advance, llmConfigured])

  // Avatar e Identidade autosalvam campo a campo (`ModelStation`/`AuroraField`, mesmo componente
  // das Configurações) — "Próximo" aqui é só navegação, sem lote para gravar.
  const handleFinishAvatar = useCallback(() => {
    setError('')
    advance('avatar', 'llm', llmConfigured)
  }, [advance, llmConfigured])

  const handleFinishIdentity = useCallback(async () => {
    setSaving(true)
    setError('')

    await runWizardSave(
      () => finishWizardSetup(),
      copy.identity.saveError,
      () => advance('identity', 'done', llmConfigured),
      setError,
      setSaving,
    )
  }, [copy.identity.saveError, advance, llmConfigured])

  return {
    locale,
    copy,
    step,
    error,
    privacyChoices,
    legalAccepted,
    retryLegal,
    ageConfirmed,
    saving,
    provider,
    model,
    apiKey,
    selectedProvider,
    modelCatalog,
    modelsLoading,
    reloadModels,
    tts,
    stt,
    memoryEnabled,
    reaction,
    llmConfigured,
    activeStepIndex,
    config,
    saveConfig,
    setError,
    setStep,
    setModel,
    setApiKey,
    setTts,
    setStt,
    setMemoryEnabled,
    handleLocaleChange,
    handleFinishLanguage,
    handleTogglePrivacy,
    setLegalAccepted,
    setAgeConfirmed,
    handleFinishPrivacy,
    handleProviderChange,
    handleFinishLlm,
    handleSkipLlm,
    handleFinishTts,
    handleFinishStt,
    handleFinishEmbedding,
    handleFinishAvatar,
    handleFinishIdentity,
  }
}
