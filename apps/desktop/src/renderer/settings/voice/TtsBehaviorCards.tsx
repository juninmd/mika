import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, GlassSlider, GlassToggle, SectionHeader } from '@mika/sdk-ui/settings'
import { fixed } from '../../src/lib/format-number'
import { ControlTile } from './control-tile'
import type { TtsFields } from './tts-provider-fields'

type T = (key: keyof TranslationDict) => string

type TtsBehaviorCardsProps = {
  t: T
  fields: TtsFields
  boolSave: (p: string, v: boolean) => Promise<void>
  numSave: (p: string, v: number) => Promise<void>
}

function SpeechCard({ t, fields, boolSave, numSave }: TtsBehaviorCardsProps) {
  const { ttsEnabled, autoSpeak, autoMuteDuringTts, volume } = fields
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.voiceTts.panel.speechSection')}
        description={t('settings.voiceTts.panel.speechSectionHint')}
        className="mb-3"
      />
      <div className="grid gap-3 sm:grid-cols-2">
        <ControlTile>
          <GlassToggle
            label={t('settings.voiceTts.panel.ttsEnabled')}
            description={t('settings.voiceTts.panel.ttsEnabledHint')}
            configPath="voice.ttsEnabled"
            value={ttsEnabled}
            onSave={boolSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassToggle
            label={t('settings.voiceTts.panel.autoSpeak')}
            description={t('settings.voiceTts.panel.autoSpeakHint')}
            configPath="voice.autoSpeak"
            value={autoSpeak}
            disabled={!ttsEnabled}
            onSave={boolSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassToggle
            label={t('settings.voiceTts.panel.autoMuteDuringTts')}
            description={t('settings.voiceTts.panel.autoMuteDuringTtsHint')}
            configPath="voice.autoMuteDuringTts"
            value={autoMuteDuringTts}
            onSave={boolSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassSlider
            label={t('settings.voiceTts.panel.volume')}
            description={t('settings.voiceTts.panel.volumeHint')}
            configPath="voice.volume"
            value={volume}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => `${Math.round(v * 100)}%`}
            onSave={numSave}
          />
        </ControlTile>
      </div>
    </GlassCard>
  )
}

function InterruptionCard({ t, fields, boolSave, numSave }: TtsBehaviorCardsProps) {
  const { interruptionEnabled, interruptionThreshold, interruptionTriggerStreak, interruptionCooldownMs } = fields
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.voiceTts.panel.interruptionSection')}
        description={t('settings.voiceTts.panel.interruptionSectionHint')}
        className="mb-3"
      />
      <div className="grid gap-3 sm:grid-cols-2">
        <ControlTile>
          <GlassToggle
            label={t('settings.voiceTts.panel.interruptionEnabled')}
            description={t('settings.voiceTts.panel.interruptionEnabledHint')}
            configPath="voice.interruptionEnabled"
            value={interruptionEnabled}
            onSave={boolSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassSlider
            label={t('settings.voiceTts.panel.threshold')}
            description={t('settings.voiceTts.panel.thresholdHint')}
            configPath="voice.interruptionThreshold"
            value={interruptionThreshold}
            min={0}
            max={1}
            step={0.01}
            showValue={(v) => fixed(v, 2)}
            disabled={!interruptionEnabled}
            onSave={numSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassSlider
            label={t('settings.voiceTts.panel.hits')}
            description={t('settings.voiceTts.panel.hitsHint')}
            configPath="voice.interruptionTriggerStreak"
            value={interruptionTriggerStreak}
            min={1}
            max={12}
            step={1}
            showValue={(v) => `${Math.round(v)}`}
            disabled={!interruptionEnabled}
            onSave={numSave}
          />
        </ControlTile>
        <ControlTile>
          <GlassSlider
            label={t('settings.voiceTts.panel.cooldown')}
            description={t('settings.voiceTts.panel.cooldownHint')}
            configPath="voice.interruptionCooldownMs"
            value={interruptionCooldownMs}
            min={250}
            max={10000}
            step={250}
            showValue={(v) => `${Math.round(v)}ms`}
            disabled={!interruptionEnabled}
            onSave={numSave}
          />
        </ControlTile>
      </div>
    </GlassCard>
  )
}

// Silence detection (VAD endpointing/threshold) lives in the STT tab's "Calibrar microfone" card:
// it decides when YOUR speech ends, not how she speaks.
export function TtsBehaviorCards(props: TtsBehaviorCardsProps) {
  return (
    <>
      <SpeechCard {...props} />
      <InterruptionCard {...props} />
    </>
  )
}
