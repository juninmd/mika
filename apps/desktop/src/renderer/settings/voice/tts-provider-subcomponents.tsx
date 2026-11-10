import { GlassSlider, usePluginTest } from '@mika/sdk-ui/settings'
import type { ReactNode } from 'react'
import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { rendererLog } from '../../src/lib/renderer-logger'

export interface UseTtsAudioTestOptions {
  testFn: () => Promise<{ ok: boolean; message?: string; audio?: string }>
  runningMessage: string
  audioMimeFromBase64: (audio: string) => string
}

// Runs the TTS test and plays the resulting audio; a repeated click stops the previous audio instead of overlapping two.
export function useTtsAudioTest({ testFn, runningMessage, audioMimeFromBase64 }: UseTtsAudioTestOptions) {
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const [isPlaying, setIsPlaying] = useState(false)

  useEffect(
    () => () => {
      audioRef.current?.pause()
    },
    [],
  )

  const {
    status,
    message: result,
    runTest,
  } = usePluginTest({
    testFn,
    runningMessage,
    onAudio: (audio) => {
      audioRef.current?.pause()
      const a = new Audio(`data:${audioMimeFromBase64(audio)};base64,${audio}`)
      audioRef.current = a
      setIsPlaying(true)
      a.onended = () => setIsPlaying(false)
      a.play().catch((e) => {
        rendererLog.warn('[Test] Audio play failed:', e)
        setIsPlaying(false)
      })
    },
    resetDelayMs: 3000,
  })

  const handleTestClick = async (): Promise<void> => {
    if (isPlaying) {
      audioRef.current?.pause()
      setIsPlaying(false)
      return
    }
    await runTest()
  }

  return { status, result, isPlaying, handleTestClick }
}

export interface TtsSettingsCardProps {
  children: ReactNode
}

/** Body of the selected provider's details panel; the panel header already names the provider. */
export function TtsSettingsCard({ children }: TtsSettingsCardProps) {
  return <div>{children}</div>
}

export interface TextLimitTimeoutSlidersProps {
  className?: string
  textLimitPath: string
  textLimit: number
  timeoutPath: string
  timeoutMs: number
  onSave: (path: string, value: number) => Promise<void>
}

export function TextLimitTimeoutSliders({
  className = '',
  textLimitPath,
  textLimit,
  timeoutPath,
  timeoutMs,
  onSave,
}: TextLimitTimeoutSlidersProps) {
  const { t } = useI18n()
  return (
    <div className={`grid grid-cols-2 gap-3 ${className}`}>
      <GlassSlider
        label={t('settings.voiceTts.field.textLimit.label')}
        configPath={textLimitPath}
        value={textLimit}
        min={500}
        max={5000}
        step={100}
        showValue={(v) => `${v}`}
        onSave={onSave}
      />
      <GlassSlider
        label={t('settings.voiceTts.field.timeoutMs.label')}
        configPath={timeoutPath}
        value={timeoutMs}
        min={5000}
        max={60000}
        step={1000}
        showValue={(v) => `${v}ms`}
        onSave={onSave}
      />
    </div>
  )
}
