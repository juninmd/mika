import { BentoGrid, GlassCard, GlassSection, ProviderChoiceGrid, SectionHeader } from '@mika/sdk-ui/settings'
import { Loader2, Mic } from 'lucide-react'
import { useI18n } from '../../src/hooks/useI18n'
import { useReportedSave } from '../../src/hooks/useReportedSave'
import { useSaveState } from '../../src/hooks/useSaveState'
import { getElectronApi } from '../../src/lib/shell-api'
import { TtsBehaviorCards } from './TtsBehaviorCards'
import { TtsLanguageNotice } from './TtsLanguageNotice'
import { TtsPreprocessorCard } from './TtsPreprocessorCard'
import { ProviderSpecificFields } from './TtsProviderSpecificFields'
import { audioMimeFromBase64, providerOptions } from './tts-provider-constants'
import {
  buildTtsCfg,
  resolveTestActionLabel,
  resolveTtsFields,
  type TTSProviderPanelProps,
} from './tts-provider-fields'
import { useTtsAudioTest } from './tts-provider-subcomponents'

export type { TTSProviderPanelProps } from './tts-provider-fields'

export function TTSProviderPanel(props: TTSProviderPanelProps) {
  const { t, locale } = useI18n()
  const { onSave } = props
  const rawCfg = props.config ?? {}

  const fields = resolveTtsFields(props, rawCfg)
  const { ttsProvider } = fields

  const voiceSave = useSaveState('voice.ttsProvider')

  const doSave = useReportedSave(onSave)

  const cfg: Record<string, unknown> = buildTtsCfg(fields)

  const {
    status: testStatus,
    result: testResult,
    isPlaying: isTestAudioPlaying,
    handleTestClick,
  } = useTtsAudioTest({
    testFn: () =>
      getElectronApi().runtime.testPlugin({
        pluginId: 'mika.voice',
        config: {
          ...cfg,
          'voice.ttsProvider': ttsProvider,
          ttsProvider,
        },
      }),
    runningMessage: t('settings.voiceTts.panel.playing'),
    audioMimeFromBase64,
  })

  const numSave = onSave as (p: string, v: number) => Promise<void>
  const boolSave = onSave as (p: string, v: boolean) => Promise<void>
  const strSave = onSave as (p: string, v: string) => Promise<void>

  return (
    <GlassSection
      title={t('settings.voiceTts.panel.title')}
      icon={Mic}
      description={t('settings.voiceTts.panel.description')}
      testAction={{
        label: resolveTestActionLabel(t, testStatus, isTestAudioPlaying, testResult),
        onTest: handleTestClick,
      }}
    >
      <BentoGrid cols="auto">
        <TtsLanguageNotice t={t} locale={locale} provider={ttsProvider} piperVoice={fields.piperVoice} />

        <GlassCard span="full">
          <SectionHeader
            title={t('settings.voiceTts.providerSection.title')}
            description={t('settings.voiceTts.providerSection.description')}
            className="mb-3"
          />
          <ProviderChoiceGrid
            providers={providerOptions(t)}
            activeId={ttsProvider}
            disabled={voiceSave.isSaving}
            renderActiveIndicator={
              voiceSave.isSaving ? () => <Loader2 className="h-3 w-3 animate-spin text-accent" /> : undefined
            }
            onSelect={(provider) => {
              void doSave('voice.ttsProvider', provider.id)
            }}
            renderDetails={(provider) => (
              <ProviderSpecificFields
                t={t}
                cfg={cfg}
                onSave={onSave}
                numSave={numSave}
                strSave={strSave}
                doSave={doSave}
                {...fields}
                ttsProvider={provider.id}
              />
            )}
          />
        </GlassCard>

        <TtsBehaviorCards t={t} fields={fields} boolSave={boolSave} numSave={numSave} />

        <TtsPreprocessorCard t={t} fields={fields} boolSave={boolSave} />

        {testResult && testResult !== t('settings.voiceTts.panel.playing') && (
          <GlassCard span="full">
            <div className="flex items-center gap-2">
              {testStatus === 'ok' ? (
                <span className="text-xs font-bold text-ok">{t('settings.voiceTts.panel.testSuccess')}</span>
              ) : (
                <span className="text-xs font-bold text-danger">{testResult}</span>
              )}
            </div>
          </GlassCard>
        )}
      </BentoGrid>
    </GlassSection>
  )
}
