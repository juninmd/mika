import { startTransition, useEffect, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { useUIStore } from '../store/useUIStore'
import { speechErrorMessageKey } from './shell-commands'
import { useI18n } from './useI18n'
import { useSpeechRecognition } from './useSpeechRecognition'

type UseComposerSpeechOptions = {
  speechLang: string
  onChange: (next: string) => void
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
}

export function useComposerSpeech({ speechLang, onChange, textareaRef }: UseComposerSpeechOptions) {
  const addNotification = useUIStore((s) => s.addNotification)
  const { t } = useI18n()
  // 'nemotron', not 'electron': matches `voice.asrProvider`'s schema default
  // (config/schemas/voice.rs) — this is only the placeholder shown for the instant before
  // `getSettingsConfig` resolves, and it never picks the capture path on its own (only
  // `asrProvider === 'browser'` does), but a stale label here still read the wrong engine on the
  // durable milestone log during that window.
  const [asrProvider, setAsrProvider] = useState('nemotron')

  useEffect(() => {
    const eAPI = getElectronApi()
    eAPI.runtime
      .getSettingsConfig()
      .then(({ config }) => {
        const voiceConfig = config.voice as { asrProvider?: string } | undefined
        const provider = voiceConfig?.asrProvider ?? 'nemotron'
        setAsrProvider(provider)
      })
      .catch(() => rendererLog.warn('[useComposerSpeech] failed to load settings'))
  }, [])

  const { listening, processing, supported, start, stop } = useSpeechRecognition({
    lang: speechLang,
    asrProvider,
    onTranscript: (text) => {
      const current = textareaRef.current?.value ?? ''
      const next = current.trim() ? `${current.trim()} ${text.trim()}` : text.trim()
      startTransition(() => onChange(next))
      textareaRef.current?.focus()
    },
    onError: (error) => {
      const key = speechErrorMessageKey(error)
      addNotification(
        'error',
        key ? t(key) : `${t('shell.composer.micError')} ${error}`,
        t('shell.neuralWheel.notice.microphone'),
      )
    },
  })

  const toggleMic = () => {
    if (listening) stop()
    else start()
  }

  return { listening, processing, supported, toggleMic, stop }
}
