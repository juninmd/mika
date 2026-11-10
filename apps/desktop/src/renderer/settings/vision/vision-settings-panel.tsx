import { toDisplayError } from '@mika/sdk-ui'
import { BentoGrid, GlassSection } from '@mika/sdk-ui/settings'
import { Eye } from 'lucide-react'
import { useCallback } from 'react'
import { configBool, configNum, configStr, pick } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import { useUIStore } from '../../src/store/useUIStore'
import { VisionLiveMonitor } from './vision-live-monitor'
import { visionBlockReason } from './vision-readiness'
import { VisionDisabledHero } from './vision-settings-components'
import { getAwarenessToggles, getBehaviorToggles, getCaptureSliders } from './vision-settings-controls'
import { VisionSettingsEnabledCards } from './vision-settings-enabled-cards'
import { VisionSettingsHero } from './vision-settings-hero'

type Props = {
  enabled: boolean
  provider: string
  model: string
  captureQuality: number
  adaptiveCompression: boolean
  maxImageSizeKB: number
  compressionTargetSizeKB: number
  llmQuality: number
  maxImageDimension: number
  maxOutputTokens: number
  captureIntervalMs: number
  analysisMode: string
  autoDescribeScreen: boolean
  visionAssistActive: boolean
  visionAssistAutoComment: boolean
  codeAwareness: boolean
  gameAwareness: boolean
  proactiveCommentaryChance: number
  readClipboardImages: boolean
  visualDiaryEnabled: boolean
  visualDiaryIntervalCycles: number
  textDensityThreshold: number
  captureBlocklist: string
  chatProvider: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export type VisionSettingsPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function VisionSettingsPanel(props: VisionSettingsPanelProps) {
  const { t } = useI18n()
  const { onSave } = props
  const rawCfg = props.config ?? {}

  const enabled = pick(props.enabled, () => configBool(rawCfg, 'vision.enabled'))
  const provider = pick(props.provider, () => configStr(rawCfg, 'vision.provider', 'default'))
  const model = pick(props.model, () => configStr(rawCfg, 'vision.model'))
  const captureQuality = pick(props.captureQuality, () => configNum(rawCfg, 'vision.captureQuality', 80))
  const adaptiveCompression = pick(props.adaptiveCompression, () =>
    configBool(rawCfg, 'vision.adaptiveCompression', true),
  )
  const maxImageSizeKB = pick(props.maxImageSizeKB, () => configNum(rawCfg, 'vision.maxImageSizeKB', 500))
  const compressionTargetSizeKB = pick(props.compressionTargetSizeKB, () =>
    configNum(rawCfg, 'vision.compressionTargetSizeKB', 300),
  )
  const llmQuality = pick(props.llmQuality, () => configNum(rawCfg, 'vision.llmQuality', 60))
  const maxImageDimension = pick(props.maxImageDimension, () => configNum(rawCfg, 'vision.maxImageDimension', 1280))
  const maxOutputTokens = pick(props.maxOutputTokens, () => configNum(rawCfg, 'vision.maxOutputTokens', 512))
  const captureIntervalMs = pick(props.captureIntervalMs, () => configNum(rawCfg, 'vision.captureIntervalMs', 60000))
  const analysisMode = pick(props.analysisMode, () => configStr(rawCfg, 'vision.analysisMode', 'balanced'))
  const autoDescribeScreen = pick(props.autoDescribeScreen, () => configBool(rawCfg, 'vision.autoDescribeScreen'))
  const visionAssistActive = pick(props.visionAssistActive, () => configBool(rawCfg, 'vision.visionAssistActive'))
  const visionAssistAutoComment = pick(props.visionAssistAutoComment, () =>
    configBool(rawCfg, 'vision.visionAssistAutoComment'),
  )
  const codeAwareness = pick(props.codeAwareness, () => configBool(rawCfg, 'vision.codeAwareness'))
  const gameAwareness = pick(props.gameAwareness, () => configBool(rawCfg, 'vision.gameAwareness'))
  const proactiveCommentaryChance = pick(props.proactiveCommentaryChance, () =>
    configNum(rawCfg, 'vision.proactiveCommentaryChance', 0.2),
  )
  const readClipboardImages = pick(props.readClipboardImages, () => configBool(rawCfg, 'vision.readClipboardImages'))
  const visualDiaryEnabled = pick(props.visualDiaryEnabled, () => configBool(rawCfg, 'vision.visualDiaryEnabled'))
  const visualDiaryIntervalCycles = pick(props.visualDiaryIntervalCycles, () =>
    configNum(rawCfg, 'vision.visualDiaryIntervalCycles', 12),
  )
  const textDensityThreshold = pick(props.textDensityThreshold, () =>
    configNum(rawCfg, 'vision.textDensityThreshold', 0.12),
  )
  const captureBlocklist = pick(props.captureBlocklist, () => configStr(rawCfg, 'vision.captureBlocklist', ''))
  const privacy = {
    screenCapture: configBool(rawCfg, 'privacy.screenCapture'),
    clipboardLogging: configBool(rawCfg, 'privacy.clipboardLogging'),
    readClipboardImages,
  }
  const blockReason = visionBlockReason(privacy)
  const chatProvider = pick(props.chatProvider, () => configStr(rawCfg, 'llm.provider', 'google'))

  const fieldConfig = {
    'vision.provider': provider,
    'vision.model': model,
    'vision.analysisMode': analysisMode,
    'vision.captureBlocklist': captureBlocklist,
  }

  const boolSave = onSave as (p: string, v: boolean) => Promise<void>
  const numSave = onSave as (p: string, v: number) => Promise<void>
  const stringSave = onSave as (p: string, v: string) => Promise<void>

  const handleEnable = useCallback(async () => {
    try {
      await onSave('vision.enabled', true)
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.vision.panel.enableError')))
    }
  }, [onSave, t])

  const captureSliders = getCaptureSliders(t, {
    captureQuality,
    llmQuality,
    maxImageSizeKB,
    compressionTargetSizeKB,
    maxImageDimension,
    maxOutputTokens,
    captureIntervalMs,
    proactiveCommentaryChance,
  })

  const behaviorToggles = getBehaviorToggles(t, { autoDescribeScreen, visionAssistActive })

  const awarenessToggles = getAwarenessToggles(t, {
    codeAwareness,
    gameAwareness,
    readClipboardImages,
    visualDiaryEnabled,
  })

  return (
    <GlassSection title={t('settings.vision.tab.label')} icon={Eye} badge="IA de visão">
      <BentoGrid cols="auto">
        {enabled && (
          <VisionSettingsHero
            t={t}
            provider={provider}
            analysisMode={analysisMode}
            captureIntervalMs={captureIntervalMs}
            codeAwareness={codeAwareness}
            gameAwareness={gameAwareness}
            privacy={privacy}
            onSavePrivacy={boolSave}
          />
        )}

        {enabled && (
          <VisionSettingsEnabledCards
            privacy={privacy}
            t={t}
            provider={provider}
            model={model}
            analysisMode={analysisMode}
            captureIntervalMs={captureIntervalMs}
            proactiveCommentaryChance={proactiveCommentaryChance}
            codeAwareness={codeAwareness}
            gameAwareness={gameAwareness}
            readClipboardImages={readClipboardImages}
            visualDiaryEnabled={visualDiaryEnabled}
            visionAssistActive={visionAssistActive}
            visionAssistAutoComment={visionAssistAutoComment}
            adaptiveCompression={adaptiveCompression}
            visualDiaryIntervalCycles={visualDiaryIntervalCycles}
            textDensityThreshold={textDensityThreshold}
            chatProvider={chatProvider}
            fieldConfig={fieldConfig}
            captureSliders={captureSliders}
            behaviorToggles={behaviorToggles}
            awarenessToggles={awarenessToggles}
            onSave={onSave}
            boolSave={boolSave}
            numSave={numSave}
            stringSave={stringSave}
          />
        )}

        {!enabled && <VisionDisabledHero onEnable={handleEnable} />}
      </BentoGrid>

      {enabled && (
        <GlassSection
          title={t('settings.vision.panel.liveMonitorTitle')}
          icon={Eye}
          badge="Live"
          collapsible
          defaultCollapsed={false}
          description={t('settings.vision.panel.liveMonitorDescription')}
        >
          <VisionLiveMonitor blockedReason={blockReason ? t(blockReason) : undefined} />
        </GlassSection>
      )}
    </GlassSection>
  )
}
