import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, GlassToggle, SectionHeader } from '@mika/sdk-ui/settings'
import { ControlTile } from './control-tile'
import type { TtsFields } from './tts-provider-fields'

type TtsPreprocessorCardProps = {
  t: (key: keyof TranslationDict) => string
  fields: TtsFields
  boolSave: (p: string, v: boolean) => Promise<void>
}

export function TtsPreprocessorCard({ t, fields, boolSave }: TtsPreprocessorCardProps) {
  const toggles = [
    {
      label: t('settings.voiceTts.panel.ignoreBrackets'),
      description: t('settings.voiceTts.panel.ignoreBracketsHint'),
      configPath: 'voice.ttsIgnoreBrackets',
      value: fields.ttsIgnoreBrackets,
    },
    {
      label: t('settings.voiceTts.panel.ignoreParentheses'),
      description: t('settings.voiceTts.panel.ignoreParenthesesHint'),
      configPath: 'voice.ttsIgnoreParentheses',
      value: fields.ttsIgnoreParentheses,
    },
    {
      label: t('settings.voiceTts.panel.ignoreAsterisks'),
      description: t('settings.voiceTts.panel.ignoreAsterisksHint'),
      configPath: 'voice.ttsIgnoreAsterisks',
      value: fields.ttsIgnoreAsterisks,
    },
    {
      label: t('settings.voiceTts.panel.ignoreAngleBrackets'),
      description: t('settings.voiceTts.panel.ignoreAngleBracketsHint'),
      configPath: 'voice.ttsIgnoreAngleBrackets',
      value: fields.ttsIgnoreAngleBrackets,
    },
    {
      label: t('settings.voiceTts.panel.removeEmoji'),
      description: t('settings.voiceTts.panel.removeEmojiHint'),
      configPath: 'voice.ttsRemoveEmoji',
      value: fields.ttsRemoveEmoji,
    },
  ]
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.voiceTts.panel.preprocessorSection')}
        description={t('settings.voiceTts.panel.preprocessorSectionHint')}
        className="mb-3"
      />
      <div className="grid gap-3 sm:grid-cols-2">
        {toggles.map((toggle) => (
          <ControlTile key={toggle.configPath}>
            <GlassToggle {...toggle} onSave={boolSave} />
          </ControlTile>
        ))}
      </div>
    </GlassCard>
  )
}
