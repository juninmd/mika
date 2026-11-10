import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, GlassSlider, GlassToggle, SectionHeader, StatusPill } from '@mika/sdk-ui/settings'
import { Mic, MicOff, Wand2 } from 'lucide-react'
import { type KeyboardEvent, type PointerEvent, useEffect, useRef, useState } from 'react'
import { useMicLevelMeter } from '../../src/hooks/useMicLevelMeter'
import { fixed } from '../../src/lib/format-number'
import { ControlTile } from './control-tile'
import {
  activityThresholdForLevel,
  gateLevelForThreshold,
  type MicThresholdSuggestion,
  suggestMicThresholds,
} from './mic-calibration'

type T = (key: keyof TranslationDict) => string

/** Top of the drawn scale: speech sits well under this, and the gate's whole range (≤0.24) stays wide enough to drag. */
const DISPLAY_MAX = 0.5
const CALIBRATION_MS = 3000
const THRESHOLD_PATH = 'voice.voiceActivityThreshold'

const pct = (level: number) => `${Math.min(100, (level / DISPLAY_MAX) * 100)}%`

type Props = {
  t: T
  voiceActivityThreshold: number
  vadSpeechThreshold: number
  vadEndpointingEnabled: boolean
  vadSilenceMs: number
  vadIgnoreTyping: boolean
  browserStt: boolean
  numSave: (p: string, v: number) => Promise<void>
  boolSave: (p: string, v: boolean) => Promise<void>
}

function useThresholdDrag(saved: number, save: (v: number) => Promise<void>) {
  const [draft, setDraft] = useState(saved)
  const trackRef = useRef<HTMLDivElement>(null)
  useEffect(() => setDraft(saved), [saved])

  const fromPointer = (e: PointerEvent<HTMLDivElement>) => {
    const rect = trackRef.current?.getBoundingClientRect()
    if (!rect || rect.width === 0) return draft
    const next = activityThresholdForLevel(((e.clientX - rect.left) / rect.width) * DISPLAY_MAX)
    setDraft(next)
    return next
  }
  const commit = (value: number) => {
    if (value !== saved) void save(value)
  }
  const handlers = {
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId)
      fromPointer(e)
    },
    onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) fromPointer(e)
    },
    onPointerUp: (e: PointerEvent<HTMLDivElement>) => commit(fromPointer(e)),
    onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => {
      const delta = { ArrowLeft: -0.02, ArrowDown: -0.02, ArrowRight: 0.02, ArrowUp: 0.02 }[e.key]
      if (delta === undefined) return
      e.preventDefault()
      setDraft((v) => Math.min(1, Math.max(0, Math.round((v + delta) * 100) / 100)))
    },
    onKeyUp: () => commit(draft),
  }
  return { draft, trackRef, handlers }
}

function LevelMeter({
  t,
  level,
  threshold,
  suggestion,
  drag,
}: {
  t: T
  level: number
  threshold: number
  suggestion: MicThresholdSuggestion | null
  drag: ReturnType<typeof useThresholdDrag>
}) {
  const gate = gateLevelForThreshold(threshold)
  const open = level >= gate
  return (
    <div
      ref={drag.trackRef}
      role="slider"
      tabIndex={0}
      aria-label={t('settings.voiceStt.calibration.lineLabel')}
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={threshold}
      aria-valuetext={fixed(threshold, 2)}
      {...drag.handlers}
      className="relative h-14 cursor-ew-resize touch-none select-none overflow-hidden rounded-xl border border-border-subtle bg-bg-deep focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
    >
      <div
        className="absolute inset-y-0 left-0 border-r border-border-subtle bg-surface-raise-strong"
        style={{ width: pct(gate) }}
      />
      <span className="pointer-events-none absolute bottom-1 left-2 text-micro font-bold uppercase tracking-[0.12em] text-text-subtle">
        {t('settings.voiceStt.calibration.zoneSilence')}
      </span>
      <span className="pointer-events-none absolute bottom-1 right-2 text-micro font-bold uppercase tracking-[0.12em] text-text-subtle">
        {t('settings.voiceStt.calibration.zoneSpeech')}
      </span>
      <div
        className={`absolute inset-y-3 left-0 rounded-r-md transition-[width] duration-100 ${open ? 'bg-ok' : 'bg-text-subtle'}`}
        style={{ width: pct(level) }}
      />
      {suggestion && !suggestion.noisy && (
        <div
          className="absolute inset-y-1 w-0 border-l-2 border-dashed border-info"
          style={{ left: pct(gateLevelForThreshold(suggestion.voiceActivityThreshold)) }}
        />
      )}
      <div className="absolute inset-y-0 w-0.5 bg-accent" style={{ left: pct(gate) }}>
        <span className="absolute -top-px left-1 rounded-b-md bg-accent px-1.5 py-0.5 font-mono text-micro font-bold text-bg-deep">
          {fixed(threshold, 2)}
        </span>
      </div>
    </div>
  )
}

function SuggestionNote({ t, suggestion, onApply }: { t: T; suggestion: MicThresholdSuggestion; onApply: () => void }) {
  return (
    <div
      role="status"
      className={`mt-3 flex flex-wrap items-center gap-3 rounded-xl border px-3 py-2.5 ${
        suggestion.noisy ? 'border-warn-rim bg-warn-soft' : 'border-info-rim bg-info-soft'
      }`}
    >
      <div className="min-w-0 flex-1 text-caption leading-snug text-text-main">
        <p className={`font-bold ${suggestion.noisy ? 'text-warn' : 'text-info'}`}>
          {t(suggestion.noisy ? 'settings.voiceStt.calibration.noisy' : 'settings.voiceStt.calibration.suggested')}
        </p>
        <p className="mt-0.5 font-mono text-text-muted">
          {t('settings.voiceStt.calibration.noiseFloor')} {fixed(suggestion.noiseFloor, 3)} ·{' '}
          {t('settings.voiceStt.calibration.lineLabel')} {fixed(suggestion.voiceActivityThreshold, 2)} ·{' '}
          {t('settings.voiceTts.panel.vadSpeechThreshold')} {fixed(suggestion.vadSpeechThreshold, 2)}
        </p>
      </div>
      <button
        type="button"
        onClick={onApply}
        className="shrink-0 rounded-lg border border-info-rim bg-bg-surface px-3 py-1.5 text-caption font-bold text-info transition hover:bg-info-soft active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        {t('settings.voiceStt.calibration.apply')}
      </button>
    </div>
  )
}

const BUTTON =
  'flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-caption font-bold transition active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-50'

function MeterActions({
  t,
  meter,
  sampling,
  onCalibrate,
}: {
  t: T
  meter: ReturnType<typeof useMicLevelMeter>
  sampling: boolean
  onCalibrate: () => void
}) {
  return (
    <>
      <button
        type="button"
        aria-pressed={meter.running}
        disabled={sampling}
        onClick={() => (meter.running ? meter.stop() : void meter.start())}
        className={`${BUTTON} ${
          meter.running
            ? 'border-danger-rim bg-danger-soft text-danger'
            : 'border-border-subtle bg-surface-raise text-text-main hover:border-border-strong'
        }`}
      >
        {meter.running ? <MicOff size={13} aria-hidden="true" /> : <Mic size={13} aria-hidden="true" />}
        {t(meter.running ? 'settings.voiceStt.calibration.stop' : 'settings.voiceStt.calibration.listen')}
      </button>
      <button
        type="button"
        disabled={sampling}
        onClick={onCalibrate}
        className={`${BUTTON} border-accent-rim bg-accent-soft text-accent-text hover:bg-accent/25`}
      >
        <Wand2 size={13} aria-hidden="true" />
        {t(sampling ? 'settings.voiceStt.calibration.sampling' : 'settings.voiceStt.calibration.auto')}
      </button>
    </>
  )
}

export function MicCalibrationCard(props: Props) {
  const { t, numSave, boolSave, browserStt } = props
  const meter = useMicLevelMeter()
  const drag = useThresholdDrag(props.voiceActivityThreshold, (v) => numSave(THRESHOLD_PATH, v))
  const [sampling, setSampling] = useState(false)
  const [suggestion, setSuggestion] = useState<MicThresholdSuggestion | null>(null)

  const calibrate = async () => {
    setSampling(true)
    setSuggestion(suggestMicThresholds(await meter.sample(CALIBRATION_MS)))
    setSampling(false)
  }
  const apply = async () => {
    if (!suggestion) return
    await numSave(THRESHOLD_PATH, suggestion.voiceActivityThreshold)
    await numSave('voice.vadSpeechThreshold', suggestion.vadSpeechThreshold)
    setSuggestion(null)
  }
  const open = meter.running && meter.level >= gateLevelForThreshold(drag.draft)

  return (
    <GlassCard span="full" accent>
      <SectionHeader
        title={t('settings.voiceStt.calibration.title')}
        description={t(
          browserStt ? 'settings.voiceTts.panel.vadBrowserHint' : 'settings.voiceStt.calibration.description',
        )}
        actions={<MeterActions t={t} meter={meter} sampling={sampling} onCalibrate={() => void calibrate()} />}
        className="mb-3"
      />
      <div className="mb-2 flex flex-wrap items-center gap-2">
        {meter.running && (
          <StatusPill
            variant={open ? 'ok' : 'neutral'}
            dot
            label={t(open ? 'settings.voiceStt.calibration.open' : 'settings.voiceStt.calibration.closed')}
          />
        )}
        {sampling && <StatusPill variant="pending" dot label={t('settings.voiceStt.calibration.quiet')} />}
        {meter.failed && <StatusPill variant="error" label={t('settings.voiceStt.calibration.micFailed')} />}
        <span className="text-caption text-text-muted">{t('settings.voiceStt.calibration.dragHint')}</span>
      </div>
      <LevelMeter t={t} level={meter.level} threshold={drag.draft} suggestion={suggestion} drag={drag} />
      {suggestion && <SuggestionNote t={t} suggestion={suggestion} onApply={() => void apply()} />}
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <ControlTile>
          <GlassSlider
            label={t('settings.voiceTts.panel.vadSpeechThreshold')}
            description={t('settings.voiceStt.calibration.speechThresholdHint')}
            configPath="voice.vadSpeechThreshold"
            value={props.vadSpeechThreshold}
            min={0.1}
            max={0.9}
            step={0.05}
            showValue={(v) => fixed(v, 2)}
            onSave={numSave}
          />
          <div className="mt-3">
            <GlassToggle
              label={t('settings.voiceStt.field.vadIgnoreTyping')}
              description={t('settings.voiceStt.calibration.ignoreTypingHint')}
              configPath="voice.vadIgnoreTyping"
              value={props.vadIgnoreTyping}
              disabled={browserStt}
              onSave={boolSave}
            />
          </div>
        </ControlTile>
        <ControlTile>
          <GlassToggle
            label={t('settings.voiceTts.panel.vadEndpointingEnabled')}
            description={t('settings.voiceStt.calibration.endpointingHint')}
            configPath="voice.vadEndpointingEnabled"
            value={props.vadEndpointingEnabled}
            disabled={browserStt}
            onSave={boolSave}
          />
          <div className="mt-3">
            <GlassSlider
              label={t('settings.voiceTts.panel.vadSilenceMs')}
              description={t('settings.voiceStt.calibration.silenceHint')}
              configPath="voice.vadSilenceMs"
              value={props.vadSilenceMs}
              min={500}
              max={5000}
              step={100}
              showValue={(v) => `${Math.round(v)}ms`}
              disabled={!props.vadEndpointingEnabled || browserStt}
              onSave={numSave}
            />
          </div>
        </ControlTile>
      </div>
    </GlassCard>
  )
}
