import { useEffect, useState } from 'react'
import { AUDIO_INPUT_DEVICE_KEY } from '../lib/audio-storage-keys'
import { rendererLog } from '../lib/renderer-logger'
import { readLocalStorage, writeLocalStorage } from '../lib/safe-local-storage'
import type { getElectronApi } from '../lib/shell-api'
import type { ShellConfigPatch, VadEngine } from './shell-config-sync'
import { parseAudioConfig, parseConfigChange, parsePrivacyConfig, parseVoiceConfig } from './shell-config-sync'
import { useI18n } from './useI18n'

type ElectronApi = ReturnType<typeof getElectronApi>

export function useShellVoiceConfig(eAPI: ElectronApi, ttsVolumeRef: { current: number }) {
  const [voiceConfigLoaded, setVoiceConfigLoaded] = useState(false)
  const [sttEnabled, setSttEnabled] = useState(true)
  const [inputMode, setInputMode] = useState<'ptt' | 'continuous'>('continuous')
  // Empty = `voice.asrLanguage` unset: speech follows the interface language (same rule as the Rust STT).
  const [voiceLang, setVoiceLang] = useState('')
  const { locale } = useI18n()
  const [asrProvider, setAsrProvider] = useState('electron')
  const [autoMuteDuringTts, setAutoMuteDuringTts] = useState(false)
  const [interruptionEnabled, setInterruptionEnabled] = useState(true)
  const [vadThreshold, setVadThreshold] = useState(0.5)
  // Same defaults as the schema (config/schemas/voice.rs) — what a session uses before the first
  // getSettingsConfig() resolves.
  const [vadSilenceMs, setVadSilenceMs] = useState(1200)
  const [vadEndpointingEnabled, setVadEndpointingEnabled] = useState(true)
  const [vadEngine, setVadEngine] = useState<VadEngine>('energy')
  const [vadIgnoreTyping, setVadIgnoreTyping] = useState(true)
  const [offlineMode, setOfflineMode] = useState(false)
  const [inputDeviceId, setInputDeviceId] = useState<string | undefined>(
    () => readLocalStorage(AUDIO_INPUT_DEVICE_KEY) ?? undefined,
  )

  useEffect(() => {
    // One setter per patch key, applied via a table instead of an if-chain, to keep this under the complexity gate.
    const setters: { [K in keyof ShellConfigPatch]-?: (value: NonNullable<ShellConfigPatch[K]>) => void } = {
      voiceLang: setVoiceLang,
      asrProvider: setAsrProvider,
      sttEnabled: setSttEnabled,
      inputMode: setInputMode,
      autoMuteDuringTts: setAutoMuteDuringTts,
      interruptionEnabled: setInterruptionEnabled,
      vadThreshold: setVadThreshold,
      vadSilenceMs: setVadSilenceMs,
      vadEndpointingEnabled: setVadEndpointingEnabled,
      vadEngine: setVadEngine,
      vadIgnoreTyping: setVadIgnoreTyping,
      ttsVolume: (v) => {
        ttsVolumeRef.current = v
      },
      offlineMode: setOfflineMode,
      inputDeviceId: (v) => {
        setInputDeviceId(v)
        writeLocalStorage(AUDIO_INPUT_DEVICE_KEY, v)
      },
    }
    const applyPatch = (patch: ShellConfigPatch) => {
      for (const key of Object.keys(patch) as (keyof ShellConfigPatch)[]) {
        const value = patch[key]
        if (value !== undefined) (setters[key] as (v: typeof value) => void)(value)
      }
    }

    eAPI.runtime
      .getSettingsConfig()
      .then(({ config }) => {
        applyPatch(parseVoiceConfig(config.voice as Record<string, unknown> | undefined))
        applyPatch(parseAudioConfig(config.audio as Record<string, unknown> | undefined))
        applyPatch(parsePrivacyConfig(config.privacy as Record<string, unknown> | undefined))
        setVoiceConfigLoaded(true)
      })
      .catch(() => rendererLog.warn('[useAppShell] failed to load settings config'))

    return eAPI.runtime.onConfigChanged((change) => applyPatch(parseConfigChange(change.path, change.value)))
  }, [eAPI.runtime, ttsVolumeRef])

  return {
    voiceConfigLoaded,
    sttEnabled,
    inputMode,
    voiceLang: voiceLang || locale,
    asrProvider,
    autoMuteDuringTts,
    interruptionEnabled,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
    vadEngine,
    vadIgnoreTyping,
    offlineMode,
    inputDeviceId,
  }
}
