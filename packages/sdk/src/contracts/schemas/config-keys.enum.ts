export enum ConfigKey {
  ApiKeys = 'apiKeys',
  Brain = 'brain',
  Browser = 'browser',
  Character = 'character',
  GamerCompanion = 'gamer-companion',
  LLM = 'llm',
  MCP = 'mcp',
  Memory = 'memory',
  Os = 'os',
  Mood = 'mood',
  PersonCatalog = 'person-catalog',
  Privacy = 'privacy',
  Scheduler = 'scheduler',
  Steam = 'steam',
  Vision = 'vision',
  Voice = 'voice',
  Setup = 'setup',
  Legal = 'legal',
  Stats = 'stats',

  PrivacyOfflineMode = 'privacy.offlineMode',
  PrivacyConversationRetention = 'privacy.conversationRetention',
  PrivacyScreenCapture = 'privacy.screenCapture',
  PrivacyClipboardLogging = 'privacy.clipboardLogging',

  TelemetryEnabled = 'telemetry.enabled',
  TelemetrySessionId = 'telemetry.sessionId',
  TelemetryOptInShown = 'telemetry.optInShown',

  AppLocale = 'app.locale',
  AppPerformanceProfile = 'app.performanceProfile',
  AppInferenceAcceleration = 'app.inferenceAcceleration',
  AppFullscreen = 'app.fullscreen',

  SystemLogLevel = 'system.logLevel',

  LlmProvider = 'llm.provider',
  LlmModel = 'llm.model',
  LlmMaxTokens = 'llm.maxTokens',
  LlmTemperature = 'llm.temperature',
  LlmHistoryLimit = 'llm.historyLimit',
  LlmMaxToolSteps = 'llm.maxToolSteps',
  LlmTopP = 'llm.topP',
  LlmSystemPrompt = 'llm.systemPrompt',
  LlmGoogleModel = 'llm.googleModel',
  LlmOpenrouterModel = 'llm.openrouterModel',
  LlmOpenaiCompatibleModel = 'llm.openaiCompatibleModel',
  LlmUnslothModel = 'llm.unslothModel',
  LlmOllamaModel = 'llm.ollamaModel',
  LlmEmbeddingProvider = 'llm.embeddingProvider',
  LlmEmbeddingModel = 'llm.embeddingModel',
  LlmEnableMonologue = 'llm.enableMonologue',
  LlmEnableThinking = 'llm.enableThinking',
  LlmThinkingMaxTokens = 'llm.thinkingMaxTokens',

  GoogleAI = 'apiKeys.googleAI',
  OpenRouter = 'apiKeys.openrouter',

  OllamaBaseUrl = 'apiKeys.ollamaBaseUrl',

  OllamaApiKey = 'apiKeys.ollamaApiKey',
  OpenAiCompatibleBaseUrl = 'apiKeys.openaiCompatibleBaseUrl',
  OpenAiCompatibleApiKey = 'apiKeys.openaiCompatibleApiKey',
  UnslothBaseUrl = 'apiKeys.unslothBaseUrl',
  UnslothApiKey = 'apiKeys.unslothApiKey',
  ElevenLabs = 'apiKeys.elevenLabs',

  VoiceGroqApiKey = 'voice.groqApiKey',
  VoiceOpenaiApiKey = 'voice.openaiApiKey',
  VoiceOpenaiTtsApiKey = 'voice.openaiTtsApiKey',
  VoiceOpenaiTtsBaseUrl = 'voice.openaiTtsBaseUrl',

  GamerCompanionSteamApiKey = 'gamer-companion.steamApiKey',

  VoiceTtsEnabled = 'voice.ttsEnabled',
  VoiceVolume = 'voice.volume',
  VoiceTtsProvider = 'voice.ttsProvider',
  VoiceSttEnabled = 'voice.sttEnabled',
  VoiceAutoSpeak = 'voice.autoSpeak',
  VoiceActivityThreshold = 'voice.voiceActivityThreshold',

  VoiceInterruptionEnabled = 'voice.interruptionEnabled',
  VoiceInterruptionThreshold = 'voice.interruptionThreshold',
  VoiceInterruptionTriggerStreak = 'voice.interruptionTriggerStreak',
  VoiceInterruptionCooldownMs = 'voice.interruptionCooldownMs',
  VoiceVadSilenceMs = 'voice.vadSilenceMs',
  VoiceVadEndpointingEnabled = 'voice.vadEndpointingEnabled',
  VoiceVadIgnoreTyping = 'voice.vadIgnoreTyping',
  VoiceTtsIgnoreBrackets = 'voice.ttsIgnoreBrackets',
  VoiceTtsIgnoreParentheses = 'voice.ttsIgnoreParentheses',
  VoiceTtsIgnoreAsterisks = 'voice.ttsIgnoreAsterisks',
  VoiceTtsIgnoreAngleBrackets = 'voice.ttsIgnoreAngleBrackets',
  VoiceTtsRemoveEmoji = 'voice.ttsRemoveEmoji',
  VoiceAzureSubscriptionKey = 'voice.azureTtsSubscriptionKey',
  VoiceAzureRegion = 'voice.azureTtsRegion',
  VoiceAzureVoice = 'voice.azureTtsVoice',
  VoiceAzureTextLimit = 'voice.azureTtsTextLimit',
  VoiceAzureTimeoutMs = 'voice.azureTtsTimeoutMs',
  VoiceGoogleApiKey = 'voice.googleTtsApiKey',
  VoiceGoogleVoice = 'voice.googleTtsVoice',
  VoiceGoogleLanguageCode = 'voice.googleTtsLanguageCode',
  VoiceGoogleSpeakingRate = 'voice.googleTtsSpeakingRate',
  VoiceGooglePitch = 'voice.googleTtsPitch',
  VoiceGoogleTextLimit = 'voice.googleTtsTextLimit',
  VoiceGoogleTimeoutMs = 'voice.googleTtsTimeoutMs',

  VisionEnabled = 'vision.enabled',
  VisionCaptureBlocklist = 'vision.captureBlocklist',

  SetupWizardCompleted = 'setup.wizardCompleted',
  SetupWizardCurrentStep = 'setup.wizardCurrentStep',

  LegalAcceptedVersion = 'legal.acceptedVersion',
  LegalAcceptedAt = 'legal.acceptedAt',
  LegalAgeConfirmed = 'legal.ageConfirmed',

  CharacterName = 'character.name',
  CharacterPersonality = 'character.personality',
  CharacterNicknamesForUser = 'character.nicknamesForUser',
  CharacterTheme = 'character.theme',
  CharacterGreetingStyle = 'character.greetingStyle',
  CharacterResponseStyle = 'character.responseStyle',
  CharacterVisualDescription = 'character.visualDescription',
  CharacterRelationshipStatus = 'character.relationshipStatus',
  CharacterHobbies = 'character.hobbies',
  CharacterDislikes = 'character.dislikes',
  CharacterSecretGoals = 'character.secretGoals',
  CharacterCustomInstructions = 'character.customInstructions',
  CharacterMoodBaseline = 'character.moodBaseline',
  CharacterEnergyLevel = 'character.energyLevel',
  CharacterFormalityLevel = 'character.formalityLevel',
  CharacterHumorStyle = 'character.humorStyle',
  CharacterSlangUsage = 'character.slangUsage',
  CharacterEmojiUsage = 'character.emojiUsage',
  CharacterJokeFrequency = 'character.jokeFrequency',
  CharacterProactiveness = 'character.proactiveness',
  CharacterEmpathyLevel = 'character.empathyLevel',
  CharacterSwearTolerance = 'character.swearTolerance',
  CharacterResponseLength = 'character.responseLength',
  CharacterMemoryRecall = 'character.memoryRecall',

  LlmEmbeddingTaskType = 'llm.embeddingTaskType',
  LlmEmbeddingOutputDimensionality = 'llm.embeddingOutputDimensionality',
}

const CREDENTIAL_SUFFIX_RE = /(token|secret|password|key)$/i
const URL_SUFFIX_RE = /url$/i
const API_KEYS_PREFIX = `${ConfigKey.ApiKeys}.`

export const SENSITIVE_CONFIG_PATHS: readonly string[] = Object.values(ConfigKey).filter(
  (path) => CREDENTIAL_SUFFIX_RE.test(path) || (path.startsWith(API_KEYS_PREFIX) && !URL_SUFFIX_RE.test(path)),
)

const CONNECTION_URL_SUFFIX_RE = /(baseurl|redisurl|endpoint)$/i
const CREDENTIAL_SUBSTRING_RE = /(authorization|cookie|private[_-]?key|credential|connection[_-]?string|dsn)/i

export function isCredentialConfigPath(path: string): boolean {
  if (CREDENTIAL_SUFFIX_RE.test(path)) return true
  if (CONNECTION_URL_SUFFIX_RE.test(path)) return true
  if (path.startsWith(API_KEYS_PREFIX)) return true

  return CREDENTIAL_SUBSTRING_RE.test(path)
}

const SCREEN_VISIBLE_ENDPOINT_PATHS: readonly string[] = [
  ConfigKey.OllamaBaseUrl,
  ConfigKey.OpenAiCompatibleBaseUrl,
  ConfigKey.UnslothBaseUrl,
  ConfigKey.VoiceOpenaiTtsBaseUrl,
]

export function isMaskedOnScreenConfigPath(path: string): boolean {
  if (SCREEN_VISIBLE_ENDPOINT_PATHS.includes(path)) return false
  return isCredentialConfigPath(path)
}
