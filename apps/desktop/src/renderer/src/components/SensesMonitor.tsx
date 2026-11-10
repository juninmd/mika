import { Brain, Ear, Eye, MessageCircle, Save, ScanText } from 'lucide-react'
import { memo, useEffect, useState } from 'react'
import { useActivity } from '../hooks/useActivity'
import type { Translate } from '../hooks/useI18n'
import { useI18n } from '../hooks/useI18n'
import { useTimeoutRef } from '../hooks/useTransientFlag'
import { getElectronApi } from '../lib/shell-api'
import { SenseDot, type SenseStatus } from './SenseDot'

type SensesState = {
  vision: SenseStatus
  visionApp: string
  visionLastAt: number | null
  listen: SenseStatus
  speak: SenseStatus
  brain: SenseStatus
  brainTool: string | null
  memory: SenseStatus
}

const FADE_MS = 4000

function listenStatus(micEnabled: boolean, vadIsSpeaking: boolean): SenseStatus {
  return !micEnabled ? 'off' : vadIsSpeaking ? 'active' : 'idle'
}

function buildSenseLabels(t: Translate, senses: SensesState, visionEnabled: boolean, memoryEnabled: boolean) {
  const visionStatusWord = visionEnabled
    ? t(senses.vision === 'active' ? 'shell.senses.vision.active' : 'shell.senses.vision.idle')
    : t('shell.senses.vision.off')
  const listenStatusWord = t(
    senses.listen === 'active'
      ? 'shell.senses.listen.active'
      : senses.listen === 'idle'
        ? 'shell.senses.listen.idle'
        : 'shell.senses.listen.off',
  )
  const speakStatusWord = t(senses.speak === 'active' ? 'shell.senses.speak.active' : 'shell.senses.speak.idle')
  const brainStatusWord = t(senses.brain === 'active' ? 'shell.senses.brain.active' : 'shell.senses.brain.idle')
  const memoryStatusWord = t(memoryEnabled ? 'shell.senses.memory.active' : 'shell.senses.memory.off')

  return {
    vision: t('shell.senses.vision.label').replace('{{status}}', visionStatusWord),
    listen: t('shell.senses.listen.label').replace('{{status}}', listenStatusWord),
    speak: t('shell.senses.speak.label').replace('{{status}}', speakStatusWord),
    brain: t('shell.senses.brain.label').replace('{{status}}', brainStatusWord),
    memory: t('shell.senses.memory.label').replace('{{status}}', memoryStatusWord),
  }
}

/** O que o tooltip do olho mostra: a região sendo lida, ou o app observado e há quanto tempo. */
function buildVisionDetail(input: {
  visionEnabled: boolean
  isReadingText: boolean
  ocrRegion?: string
  app: string
  ago?: string
}): string | undefined {
  if (!input.visionEnabled) return undefined
  if (input.isReadingText) return input.ocrRegion
  if (!input.app) return input.ago
  return input.ago ? `${input.app} · ${input.ago}` : input.app
}

type Props = {
  micEnabled: boolean
  vadIsSpeaking: boolean
  ttsIsPlaying: boolean
  visionEnabled: boolean
  memoryEnabled: boolean
  appStatus: string
  activeTool: { name: string } | null
}

/**
 * "Pausar mic e tela" da bandeja vive no Rust (`privacy/senses.rs`): o áudio e a captura são
 * descartados lá, e este evento é o único jeito de o HUD saber — sem ele o monitor seguia "ouvindo".
 */
function useSensesPaused(): boolean {
  const [paused, setPaused] = useState(false)
  useEffect(() => getElectronApi()?.runtime?.onSensesPaused?.((payload) => setPaused(payload.paused === true)), [])
  return paused
}

export const SensesMonitor = memo(function SensesMonitor({
  micEnabled: micConfigured,
  vadIsSpeaking,
  ttsIsPlaying,
  visionEnabled: visionConfigured,
  memoryEnabled,
  appStatus,
  activeTool,
}: Props) {
  const { t } = useI18n()
  const sensesPaused = useSensesPaused()
  const micEnabled = micConfigured && !sensesPaused
  const visionEnabled = visionConfigured && !sensesPaused
  const [senses, setSenses] = useState<SensesState>({
    vision: 'off',
    visionApp: '',
    visionLastAt: null,
    listen: 'off',
    speak: 'off',
    brain: 'idle',
    brainTool: null,
    memory: 'idle',
  })
  const { set: setVisionFade, clear: clearVisionFade } = useTimeoutRef()

  const activity = useActivity()

  useEffect(() => {
    setSenses((s) => ({
      ...s,
      listen: listenStatus(micEnabled, vadIsSpeaking || activity.voice.state === 'listening'),
      // `speaking` vem do main; `ttsIsPlaying` é o áudio tocando aqui. Os dois juntos cobrem a
      // janela entre "a síntese começou" e "o primeiro buffer chegou ao alto-falante".
      speak: ttsIsPlaying || activity.voice.state === 'speaking' ? 'active' : 'idle',
      brain: appStatus === 'thinking' || activity.voice.state === 'thinking' ? 'active' : 'idle',
      brainTool: activeTool?.name ?? null,
      memory: memoryEnabled ? 'idle' : 'off',
    }))
  }, [micEnabled, vadIsSpeaking, ttsIsPlaying, memoryEnabled, appStatus, activeTool?.name, activity.voice.state])

  // O ciclo de visão acende o indicador em TODA captura, não só nas que geram comentário. O
  // `onScreenObservation` abaixo continua servindo para dizer QUAL app está sendo observado.
  const visionBusy = activity.vision.state !== 'idle'
  const isReadingText = activity.vision.state === 'ocr'

  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI?.runtime?.onScreenObservation) return

    const unsub = eAPI.runtime.onScreenObservation(({ foregroundApp, at }) => {
      clearVisionFade()
      setSenses((s) => ({
        ...s,
        vision: 'active',
        visionApp: foregroundApp?.title ?? foregroundApp?.processName ?? '',
        visionLastAt: at ?? Date.now(),
      }))
      setVisionFade(() => {
        setSenses((s) => ({ ...s, vision: visionEnabled ? 'idle' : 'off' }))
      }, FADE_MS)
    })

    return () => {
      unsub()
      clearVisionFade()
    }
  }, [visionEnabled, clearVisionFade, setVisionFade])

  const ago = senses.visionLastAt
    ? t('shell.senses.ago').replace('{{s}}', String(Math.round((Date.now() - senses.visionLastAt) / 1000)))
    : undefined

  const labels = buildSenseLabels(t, senses, visionEnabled, memoryEnabled)
  const visionDetail = buildVisionDetail({
    visionEnabled,
    isReadingText,
    ocrRegion: activity.vision.detail,
    app: senses.visionApp,
    ago,
  })

  return (
    <div
      className="flex items-center gap-2 rounded-full border border-border-subtle bg-bg-surface/80 px-3 py-1.5 backdrop-blur-md shadow-md shadow-black/20 hover:border-border-strong transition-colors"
      role="status"
      aria-label={t('shell.senses.aria')}
    >
      <SenseDot
        icon={isReadingText ? ScanText : Eye}
        status={visionEnabled ? (visionBusy ? 'active' : senses.vision) : 'off'}
        label={isReadingText ? t('shell.senses.ocr') : labels.vision}
        detail={visionDetail}
      />
      <SenseDot icon={Ear} status={senses.listen} label={labels.listen} />
      <SenseDot icon={MessageCircle} status={senses.speak} label={labels.speak} />
      <SenseDot icon={Brain} status={senses.brain} label={labels.brain} detail={senses.brainTool ?? undefined} />
      <SenseDot icon={Save} status={memoryEnabled ? senses.memory : 'off'} label={labels.memory} />
    </div>
  )
})
