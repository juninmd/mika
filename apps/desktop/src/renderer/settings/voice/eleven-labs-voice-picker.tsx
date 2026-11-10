import { AuroraField } from '@mika/sdk-ui/settings'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { rendererLog } from '../../src/lib/renderer-logger'
import { getElectronApi } from '../../src/lib/shell-api'
import { ElevenLabsVoiceOption } from './ElevenLabsVoiceOption'
import { type ElevenVoice, errorMessage, type Gender } from './eleven-labs-voice'

type Props = {
  apiKey: string
  selectedId: string
  onSelect: (voiceId: string) => void
}

export function ElevenLabsVoicePicker({ apiKey, selectedId, onSelect }: Props) {
  const { t } = useI18n()
  const [voices, setVoices] = useState<ElevenVoice[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [gender, setGender] = useState<Gender>('all')
  const [playing, setPlaying] = useState<string | null>(null)
  const [manualId, setManualId] = useState(selectedId)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  useEffect(() => {
    setManualId(selectedId)
  }, [selectedId])

  useEffect(
    () => () => {
      audioRef.current?.pause()
    },
    [],
  )

  useEffect(() => {
    if (!apiKey) return
    setLoading(true)
    setError(null)
    const api = getElectronApi()
    api.runtime
      .getElevenLabsVoices({ apiKey })
      .then((res) => {
        if (!res.ok) throw new Error(res.error ?? t('settings.voiceTts.panel.elevenLabs.unknownError'))
        setVoices((res.voices ?? []) as ElevenVoice[])
      })
      .catch((e: unknown) => {
        rendererLog.error('[ElevenLabsVoicePicker] failed to load voices', errorMessage(e))
        setError(t('settings.voiceTts.panel.elevenLabs.loadError'))
      })
      .finally(() => setLoading(false))
  }, [apiKey, t])

  const filtered = useMemo(() => {
    const q = search.toLowerCase()
    return voices.filter((v) => {
      if (gender !== 'all' && v.labels.gender !== gender) return false
      if (q && !v.name.toLowerCase().includes(q)) return false
      return true
    })
  }, [voices, gender, search])

  const playPreview = (v: ElevenVoice, e: React.MouseEvent) => {
    e.stopPropagation()
    audioRef.current?.pause()
    if (playing === v.voice_id) {
      setPlaying(null)
      return
    }
    const audio = new Audio(v.preview_url)
    audioRef.current = audio
    setPlaying(v.voice_id)
    audio.play().catch(() => setPlaying(null))
    audio.onended = () => setPlaying(null)
  }

  if (!apiKey) {
    return <p className="text-xs text-text-muted py-2">{t('settings.voiceTts.panel.elevenLabs.needsApiKey')}</p>
  }
  if (loading) {
    return (
      <p className="text-xs text-warn/60 py-2 animate-pulse">{t('settings.voiceTts.panel.elevenLabs.loadingVoices')}</p>
    )
  }
  if (error) {
    return <p className="text-xs text-danger py-2">{error}</p>
  }

  return (
    <div className="space-y-3">
      <div className="flex gap-2 flex-wrap items-start">
        <div className="flex-1 min-w-0">
          <AuroraField
            label={t('settings.voiceTts.panel.elevenLabs.searchPlaceholder')}
            hideLabel
            type="search"
            value={search}
            onChange={setSearch}
            placeholder={t('settings.voiceTts.panel.elevenLabs.searchPlaceholder')}
          />
        </div>
        {(['all', 'female', 'male'] as const).map((g) => (
          <button
            key={g}
            type="button"
            onClick={() => setGender(g)}
            className={`rounded-lg border px-3 py-2 text-micro font-bold uppercase tracking-wider transition ${
              gender === g
                ? 'border-warn-rim bg-warn-soft text-warn'
                : 'border-border-subtle bg-surface-raise text-text-muted hover:text-text-main'
            }`}
          >
            {g === 'all'
              ? t('settings.voiceTts.panel.elevenLabs.genderAll')
              : g === 'female'
                ? t('settings.voiceTts.panel.elevenLabs.genderFemale')
                : t('settings.voiceTts.panel.elevenLabs.genderMale')}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-2 gap-2 max-h-72 overflow-y-auto pr-1 scrollbar-thin" role="listbox">
        {filtered.map((v) => {
          const selected = selectedId === v.voice_id
          const isPlaying = playing === v.voice_id

          return (
            <ElevenLabsVoiceOption
              key={v.voice_id}
              t={t}
              v={v}
              selected={selected}
              isPlaying={isPlaying}
              onSelect={onSelect}
              playPreview={playPreview}
            />
          )
        })}
        {filtered.length === 0 && (
          <p className="col-span-2 py-6 text-center text-xs text-text-muted">
            {t('settings.voiceTts.panel.elevenLabs.noVoicesFound')}
          </p>
        )}
      </div>

      <AuroraField
        label={t('settings.voiceTts.panel.elevenLabs.manualIdAriaLabel')}
        hideLabel
        value={manualId}
        onChange={setManualId}
        onSubmit={() => manualId.trim() && onSelect(manualId.trim())}
        onCommit={() => {
          if (manualId.trim() && manualId !== selectedId) onSelect(manualId.trim())
        }}
        placeholder={t('settings.voiceTts.panel.elevenLabs.manualIdPlaceholder')}
        trailing={
          <button
            type="button"
            onClick={() => {
              onSelect('EXAVITQu4vr4xnSDxMaL')
              setManualId('EXAVITQu4vr4xnSDxMaL')
            }}
            title={t('settings.voiceTts.panel.elevenLabs.restoreDefaultTitle')}
            className="shrink-0 rounded-lg border border-border-subtle px-2 py-2 text-micro font-bold uppercase tracking-wider text-text-muted hover:border-warn-rim hover:text-warn/70 transition focus-visible:ring-2 focus-visible:ring-warn focus-visible:outline-none"
          >
            {t('settings.voiceTts.panel.elevenLabs.defaultButton')}
          </button>
        }
      />
    </div>
  )
}
