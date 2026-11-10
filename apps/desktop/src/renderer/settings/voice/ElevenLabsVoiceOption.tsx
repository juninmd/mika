import type { TranslationDict } from '@mika/sdk/i18n'
import { useI18n } from '../../src/hooks/useI18n'
import type { ElevenVoice } from './eleven-labs-voice'

function VoiceBadges({ voice }: { voice: ElevenVoice }) {
  const { t } = useI18n()
  const chip = 'rounded px-1 py-0.5 text-micro font-bold uppercase tracking-wider'
  const neutral = `${chip} bg-surface-raise text-text-muted`
  const availabilityBadge: Record<string, { text: string; className: string }> = {
    'plan-restricted': {
      text: t('settings.voiceTts.panel.elevenLabs.planRestricted'),
      className: 'bg-danger-soft text-danger',
    },
    unknown: {
      text: t('settings.voiceTts.panel.elevenLabs.unconfirmed'),
      className: 'bg-warn-soft text-warn/80',
    },
  }
  const availability = voice.availability ? availabilityBadge[voice.availability] : undefined

  return (
    <div className="mt-1.5 flex flex-wrap gap-1">
      {voice.labels.gender ? (
        <span
          className={`${chip} ${voice.labels.gender === 'female' ? 'bg-accent-soft text-accent' : 'bg-info-soft text-info'}`}
        >
          {voice.labels.gender === 'female'
            ? t('settings.voiceTts.panel.elevenLabs.genderFemale')
            : t('settings.voiceTts.panel.elevenLabs.genderMale')}
        </span>
      ) : null}
      {voice.labels.accent ? <span className={neutral}>{voice.labels.accent}</span> : null}
      {voice.labels.use_case ? <span className={neutral}>{voice.labels.use_case}</span> : null}
      {voice.category === 'cloned' ? (
        <span className={`${chip} bg-accent-soft text-accent`}>{t('settings.voiceTts.panel.elevenLabs.clone')}</span>
      ) : null}
      {availability ? (
        <span title={voice.availabilityLabel} className={`${chip} ${availability.className}`}>
          {availability.text}
        </span>
      ) : null}
    </div>
  )
}

type ElevenLabsVoiceOptionProps = {
  t: (key: keyof TranslationDict) => string
  v: ElevenVoice
  selected: boolean
  isPlaying: boolean
  onSelect: (voiceId: string) => void
  playPreview: (v: ElevenVoice, e: React.MouseEvent) => void
}

export function ElevenLabsVoiceOption({
  t,
  v,
  selected,
  isPlaying,
  onSelect,
  playPreview,
}: ElevenLabsVoiceOptionProps) {
  return (
    <div
      role="option"
      tabIndex={0}
      aria-selected={selected}
      onClick={() => onSelect(v.voice_id)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect(v.voice_id)
        }
      }}
      className={`relative rounded-xl border p-3 text-left transition cursor-pointer focus-visible:ring-2 focus-visible:ring-warn focus-visible:outline-none ${
        selected
          ? 'border-warn-rim bg-warn-soft text-warn'
          : 'border-border-subtle bg-surface-raise hover:border-border-strong hover:bg-surface-raise-strong text-text-main'
      }`}
    >
      <div className="flex items-start justify-between gap-1">
        <span className="text-xs font-bold leading-tight line-clamp-1">{v.name}</span>
        <button
          type="button"
          onClick={(e) => playPreview(v, e)}
          aria-label={
            isPlaying
              ? t('settings.voiceTts.panel.elevenLabs.stopPreview')
              : `${t('settings.voiceTts.panel.elevenLabs.playVoicePrefix')} ${v.name}`
          }
          className="shrink-0 rounded-md border border-warn-rim p-1 text-warn/70 hover:text-warn hover:bg-warn-soft transition focus-visible:ring-2 focus-visible:ring-warn focus-visible:outline-none"
        >
          {isPlaying ? (
            <svg className="h-3 w-3" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <rect x="6" y="4" width="4" height="16" />
              <rect x="14" y="4" width="4" height="16" />
            </svg>
          ) : (
            <svg className="h-3 w-3" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <polygon points="5,3 19,12 5,21" />
            </svg>
          )}
        </button>
      </div>
      <VoiceBadges voice={v} />
      {selected && <div className="absolute right-2 top-2 h-1.5 w-1.5 rounded-full bg-warn" />}
    </div>
  )
}
