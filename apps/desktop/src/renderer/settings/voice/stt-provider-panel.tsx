import { BentoGrid, choiceOption, GlassCard, GlassSection, TestResultNote, usePluginTest } from '@mika/sdk-ui/settings'
import { Gauge, Hand, Mic, Rabbit, Radio, Target } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { configBool, configNum, configStr, pick } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import { useReportedSave } from '../../src/hooks/useReportedSave'
import { getElectronApi } from '../../src/lib/shell-api'
import { ListenActivityPanel } from './listen-activity-panel'
import { SttSettingsCards } from './SttSettingsCards'

type Props = {
  sttEnabled: boolean
  voiceActivityThreshold: number
  inputMode: 'ptt' | 'continuous'
  asrProvider: string
  asrLanguage: string
  whisperModel: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export type STTProviderPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function STTProviderPanel(props: STTProviderPanelProps) {
  const { t } = useI18n()
  const { onSave } = props
  const rawCfg = props.config ?? {}

  const WHISPER_MODEL_OPTIONS = [
    choiceOption(
      t('settings.voiceStt.panel.whisperModel.option.tiny'),
      'onnx-community/whisper-tiny',
      t('settings.voiceStt.panel.whisperModel.hint.tiny'),
      Rabbit,
    ),
    choiceOption(
      t('settings.voiceStt.panel.whisperModel.option.base'),
      'onnx-community/whisper-base',
      t('settings.voiceStt.panel.whisperModel.hint.base'),
      Gauge,
    ),
    choiceOption(
      t('settings.voiceStt.panel.whisperModel.option.small'),
      'onnx-community/whisper-small',
      t('settings.voiceStt.panel.whisperModel.hint.small'),
      Target,
    ),
  ]

  const INPUT_MODE_OPTIONS = [
    choiceOption(
      t('settings.voiceStt.panel.inputMode.option.continuous'),
      'continuous',
      t('settings.voiceStt.panel.inputMode.hint.continuous'),
      Radio,
    ),
    choiceOption(
      t('settings.voiceStt.panel.inputMode.option.ptt'),
      'ptt',
      t('settings.voiceStt.panel.inputMode.hint.ptt'),
      Hand,
    ),
  ]

  const sttEnabled = pick(props.sttEnabled, () => configBool(rawCfg, 'voice.sttEnabled'))
  const voiceActivityThreshold = pick(props.voiceActivityThreshold, () =>
    configNum(rawCfg, 'voice.voiceActivityThreshold', 0.5),
  )
  const vadSpeechThreshold = configNum(rawCfg, 'voice.vadSpeechThreshold', 0.35)
  const vadEndpointingEnabled = configBool(rawCfg, 'voice.vadEndpointingEnabled', true)
  // Matches the schema default (config/schemas/voice.rs) — the main listening path's hangover.
  const vadSilenceMs = configNum(rawCfg, 'voice.vadSilenceMs', 1200)
  const inputMode = (props.inputMode ?? configStr(rawCfg, 'voice.inputMode', 'continuous')) as 'ptt' | 'continuous'
  const asrProvider = pick(props.asrProvider, () => configStr(rawCfg, 'voice.asrProvider', 'nemotron'))
  const asrLanguage = pick(props.asrLanguage, () => configStr(rawCfg, 'voice.asrLanguage', 'pt'))
  const whisperModel = pick(props.whisperModel, () =>
    configStr(rawCfg, 'voice.whisperModel', 'onnx-community/whisper-base'),
  )

  const [asrProviderDraft, setAsrProviderDraft] = useState(asrProvider)
  const [inputModeDraft, setInputModeDraft] = useState(inputMode)

  useEffect(() => {
    setAsrProviderDraft(asrProvider)
  }, [asrProvider])

  useEffect(() => {
    setInputModeDraft(inputMode)
  }, [inputMode])

  const doSave = useReportedSave(onSave)

  const selectAsrProvider = useCallback(
    async (value: string) => {
      setAsrProviderDraft(value)
      await doSave('voice.asrProvider', value)
    },
    [doSave],
  )

  const cfg: Record<string, unknown> = {
    'voice.asrLanguage': asrLanguage,
    'voice.whisperModel': whisperModel,
    'voice.asrProvider': asrProvider,
    'voice.groqSttModel': configStr(rawCfg, 'voice.groqSttModel', 'whisper-large-v3-turbo'),
    'voice.openaiSttModel': configStr(rawCfg, 'voice.openaiSttModel', 'gpt-transcribe'),
    'voice.vadEngine': configStr(rawCfg, 'voice.vadEngine', 'energy'),
    'voice.vadIgnoreTyping': configBool(rawCfg, 'voice.vadIgnoreTyping', true),
  }

  const numSave = onSave as (p: string, v: number) => Promise<void>
  const boolSave = onSave as (p: string, v: boolean) => Promise<void>

  const {
    status: testStatus,
    message: testResult,
    runTest: testSTT,
  } = usePluginTest({
    testFn: () =>
      getElectronApi().runtime.testPlugin({
        pluginId: 'mika.voice',
        config: {
          asrProvider: asrProviderDraft,
          asrLanguage,
          whisperModel,
          testType: 'stt',
        },
      }),
    runningMessage: t('settings.voiceStt.panel.testing'),
    resetDelayMs: 0,
  })

  return (
    <GlassSection
      title={t('settings.voiceStt.panel.title')}
      icon={Mic}
      description={t('settings.voiceStt.panel.description')}
      testAction={{
        label:
          testStatus === 'testing' ? t('settings.voiceStt.panel.testing') : t('settings.voiceStt.panel.testProvider'),
        onTest: testSTT,
      }}
    >
      <BentoGrid cols="auto">
        {testResult && testStatus !== 'testing' && (
          <GlassCard span="full">
            <TestResultNote status={testStatus} message={testResult} />
          </GlassCard>
        )}

        <SttSettingsCards
          t={t}
          sttEnabled={sttEnabled}
          voiceActivityThreshold={voiceActivityThreshold}
          vadSpeechThreshold={vadSpeechThreshold}
          vadEndpointingEnabled={vadEndpointingEnabled}
          vadSilenceMs={vadSilenceMs}
          inputModeDraft={inputModeDraft}
          setInputModeDraft={setInputModeDraft}
          asrProviderDraft={asrProviderDraft}
          selectAsrProvider={selectAsrProvider}
          cfg={cfg}
          onSave={onSave}
          doSave={doSave}
          numSave={numSave}
          boolSave={boolSave}
          INPUT_MODE_OPTIONS={INPUT_MODE_OPTIONS}
          WHISPER_MODEL_OPTIONS={WHISPER_MODEL_OPTIONS}
        />
      </BentoGrid>

      {sttEnabled && (
        <GlassSection
          title={t('settings.voiceStt.panel.listenActivityTitle')}
          icon={Mic}
          collapsible
          defaultCollapsed={false}
          className="mt-4"
          description={t('settings.voiceStt.panel.listenActivityDescription')}
        >
          <ListenActivityPanel asrProvider={asrProvider} />
        </GlassSection>
      )}
    </GlassSection>
  )
}
