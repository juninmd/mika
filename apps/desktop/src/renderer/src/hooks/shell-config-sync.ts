// Free of React so the key-to-field rules are asserted without mounting the hook.
export type ShellConfigPatch = Partial<{
  voiceLang: string
  asrProvider: string
  sttEnabled: boolean
  inputMode: 'ptt' | 'continuous'
  autoMuteDuringTts: boolean
  interruptionEnabled: boolean
  vadThreshold: number
  vadSilenceMs: number
  vadEndpointingEnabled: boolean
  vadEngine: VadEngine
  vadIgnoreTyping: boolean
  ttsVolume: number
  inputDeviceId: string
  offlineMode: boolean
}>

type ConfigRecord = Record<string, unknown> | undefined

/** Same choices as `voice.vadEngine` (config/schemas/voice.rs). */
export type VadEngine = 'energy' | 'silero'

function isVadEngine(v: unknown): v is VadEngine {
  return v === 'energy' || v === 'silero'
}

/** The Rust VAD only has a verdict when the mic feeds it: barge-in, the silero engine, or
 * `voice.vadIgnoreTyping` (it is what recognizes keystrokes and drops a typing-only utterance). */
export function needsNeuralVadFeed(s: {
  interruptionEnabled: boolean
  vadEngine: VadEngine
  vadIgnoreTyping: boolean
  micEnabled: boolean
}): boolean {
  return s.micEnabled && (s.interruptionEnabled || s.vadEngine === 'silero' || s.vadIgnoreTyping)
}

function isInputMode(v: unknown): v is 'ptt' | 'continuous' {
  return v === 'ptt' || v === 'continuous'
}

export function parseVoiceConfig(cfg: ConfigRecord): ShellConfigPatch {
  if (!cfg) return {}
  const patch: ShellConfigPatch = {}
  if (typeof cfg.asrLanguage === 'string') patch.voiceLang = cfg.asrLanguage
  if (typeof cfg.asrProvider === 'string') patch.asrProvider = cfg.asrProvider
  if (cfg.sttEnabled !== undefined) patch.sttEnabled = !!cfg.sttEnabled
  if (isInputMode(cfg.inputMode)) patch.inputMode = cfg.inputMode
  if (cfg.autoMuteDuringTts !== undefined) patch.autoMuteDuringTts = cfg.autoMuteDuringTts !== false
  if (cfg.interruptionEnabled !== undefined) patch.interruptionEnabled = !!cfg.interruptionEnabled
  if (typeof cfg.voiceActivityThreshold === 'number') patch.vadThreshold = cfg.voiceActivityThreshold
  if (typeof cfg.vadSilenceMs === 'number') patch.vadSilenceMs = cfg.vadSilenceMs
  if (cfg.vadEndpointingEnabled !== undefined) patch.vadEndpointingEnabled = cfg.vadEndpointingEnabled !== false
  if (isVadEngine(cfg.vadEngine)) patch.vadEngine = cfg.vadEngine
  if (cfg.vadIgnoreTyping !== undefined) patch.vadIgnoreTyping = cfg.vadIgnoreTyping !== false
  if (typeof cfg.volume === 'number') patch.ttsVolume = cfg.volume
  return patch
}

export function parseAudioConfig(cfg: ConfigRecord): ShellConfigPatch {
  return typeof cfg?.inputDeviceId === 'string' ? { inputDeviceId: cfg.inputDeviceId } : {}
}

export function parsePrivacyConfig(cfg: ConfigRecord): ShellConfigPatch {
  return cfg?.offlineMode !== undefined ? { offlineMode: !!cfg.offlineMode } : {}
}

const CHANGE_PATH_PARSERS: Record<string, (value: unknown) => ShellConfigPatch> = {
  'voice.asrLanguage': (v) => (typeof v === 'string' ? { voiceLang: v } : {}),
  'voice.asrProvider': (v) => (typeof v === 'string' ? { asrProvider: v } : {}),
  'voice.sttEnabled': (v) => ({ sttEnabled: !!v }),
  'voice.inputMode': (v) => (isInputMode(v) ? { inputMode: v } : {}),
  'voice.autoMuteDuringTts': (v) => ({ autoMuteDuringTts: v !== false }),
  'voice.interruptionEnabled': (v) => ({ interruptionEnabled: !!v }),
  'voice.voiceActivityThreshold': (v) => (typeof v === 'number' ? { vadThreshold: v } : {}),
  'voice.vadSilenceMs': (v) => (typeof v === 'number' ? { vadSilenceMs: v } : {}),
  'voice.vadEndpointingEnabled': (v) => ({ vadEndpointingEnabled: v !== false }),
  'voice.vadEngine': (v) => (isVadEngine(v) ? { vadEngine: v } : {}),
  'voice.vadIgnoreTyping': (v) => ({ vadIgnoreTyping: v !== false }),
  'voice.volume': (v) => (typeof v === 'number' ? { ttsVolume: v } : {}),
  'audio.inputDeviceId': (v) => (typeof v === 'string' ? { inputDeviceId: v } : {}),
  'privacy.offlineMode': (v) => ({ offlineMode: !!v }),
}

export function parseConfigChange(path: string, value: unknown): ShellConfigPatch {
  return CHANGE_PATH_PARSERS[path]?.(value) ?? {}
}
