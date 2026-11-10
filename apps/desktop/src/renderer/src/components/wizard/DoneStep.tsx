import { BentoGrid, GlassCard, StatusPill } from '@mika/sdk-ui/settings'
import { type ReactNode, useState } from 'react'
import { gateLevelForThreshold } from '../../../settings/voice/mic-calibration'
import { Keys } from '../../../settings/voice/shortcuts-card'
import { useMicLevelMeter } from '../../hooks/useMicLevelMeter'
import { shortcutById } from '../../lib/app-shortcuts'
import { isMacPlatform } from '../../lib/keyboard-shortcuts'
import { BrainIcon, KeyboardIcon, MicrophoneIcon, SpeakerIcon } from '../Icons'
import { AssetPreparationProgress } from './AssetPreparationProgress'
import { PrimaryButton, SecondaryButton } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'
import type { SttSelection } from './wizard-stt'
import type { TtsSelection } from './wizard-tts'

function StatusCard({
  description,
  icon,
  label,
  value,
}: {
  description: string
  icon: ReactNode
  label: string
  value: string
}) {
  return (
    <GlassCard span={1} className="flex items-start gap-3 p-3 sm:p-3.5">
      <span
        aria-hidden="true"
        className="flex size-8 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent"
      >
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-micro font-black uppercase tracking-[0.14em] text-accent">{label}</div>
        <div className="mt-0.5 text-xs font-black text-text-main">{value}</div>
        <div className="mt-0.5 text-caption leading-relaxed text-text-muted">{description}</div>
      </div>
    </GlassCard>
  )
}

// Keys come from the same catalog `shortcuts.rs` is tested against; this copy only says what they do.
const DONE_SHORTCUTS = [
  ['toggleInput', 'shortcutsChat'],
  ['toggleMic', 'shortcutsMic'],
  ['pushToTalk', 'shortcutsPushToTalk'],
  ['history', 'shortcutsHistory'],
] as const

const MIC_DISPLAY_MAX = 0.5

function MicCheck({ copy }: { copy: WizardCopy['done'] }) {
  const meter = useMicLevelMeter()
  const [heard, setHeard] = useState(false)
  // Default `voice.voiceActivityThreshold` gate: crossing it is what makes her start listening.
  const gate = gateLevelForThreshold(0.5)
  if (meter.running && !heard && meter.level >= gate) setHeard(true)
  const status = meter.failed ? copy.micCheckFailed : heard ? copy.micCheckHeard : copy.micCheckHint
  return (
    <div className="mt-2 flex items-center gap-3 rounded-xl border border-border-subtle bg-surface-raise p-2 sm:p-2.5">
      <button
        type="button"
        aria-pressed={meter.running}
        onClick={() => (meter.running ? meter.stop() : void meter.start())}
        className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border-subtle bg-bg-deep px-2.5 py-1.5 text-xs font-bold text-text-main transition hover:border-border-strong active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <MicrophoneIcon className="size-3.5" aria-hidden="true" />
        {meter.running ? copy.micCheckStop : copy.micCheckStart}
      </button>
      <div className="min-w-0 flex-1">
        <div aria-hidden="true" className="relative h-2 overflow-hidden rounded-full bg-bg-deep">
          <div
            className={`h-full rounded-full transition-[width] duration-100 ${heard ? 'bg-ok' : 'bg-accent'}`}
            style={{ width: `${Math.min(100, (meter.level / MIC_DISPLAY_MAX) * 100)}%` }}
          />
          <div
            className="absolute inset-y-0 w-0.5 bg-text-muted"
            style={{ left: `${(gate / MIC_DISPLAY_MAX) * 100}%` }}
          />
        </div>
        <p
          role="status"
          className={`mt-1 truncate text-caption ${meter.failed ? 'text-danger' : heard ? 'text-ok' : 'text-text-muted'}`}
        >
          {status}
        </p>
      </div>
    </div>
  )
}

export function DoneStep({
  copy,
  assetsCopy,
  llmConfigured,
  llmProvider,
  llmModel,
  tts,
  listening,
  stt,
  onBack,
  onComplete,
}: {
  copy: WizardCopy['done']
  assetsCopy: WizardCopy['assets']
  llmConfigured: boolean
  llmProvider: string
  llmModel: string
  tts: TtsSelection
  /** Toggle de microfone do passo de privacidade — define se os modelos de áudio local baixam. */
  listening: boolean
  stt: SttSelection
  onBack: () => void
  onComplete: () => void
}) {
  const [isReady, setIsReady] = useState(false)
  const mac = isMacPlatform()

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-2">
        <StatusPill
          variant={llmConfigured ? 'ok' : 'pending'}
          label={copy.badge}
          dot
          className="w-fit uppercase tracking-[0.14em]"
        />
      </div>
      <h2 className="mt-2 text-xl font-black tracking-tight text-text-main md:text-2xl">{copy.title}</h2>
      <p className="mt-1 text-xs leading-relaxed text-text-muted">{copy.description}</p>

      <div className="mt-3.5">
        <BentoGrid cols={2}>
          <StatusCard
            label={copy.brainLabel}
            value={llmConfigured ? copy.brainValue : copy.brainPendingValue}
            description={llmConfigured ? copy.brainDescription : copy.brainPendingDescription}
            icon={<BrainIcon className="size-3.5" aria-hidden="true" />}
          />
          <StatusCard
            label={copy.voiceLabel}
            value={copy.voiceValue}
            description={copy.voiceDescription}
            icon={<SpeakerIcon className="size-3.5" aria-hidden="true" />}
          />
        </BentoGrid>
      </div>

      <AssetPreparationProgress
        copy={assetsCopy}
        llmConfigured={llmConfigured}
        llmProvider={llmProvider}
        llmModel={llmModel}
        tts={tts}
        listening={listening}
        stt={stt}
        onReadyStateChange={setIsReady}
      />

      <MicCheck copy={copy} />

      <div className="mt-3 rounded-2xl border border-border-subtle bg-surface-raise p-3 sm:p-3.5">
        <div className="flex items-center gap-2 text-micro font-black uppercase tracking-[0.14em] text-accent">
          <KeyboardIcon className="size-3.5" aria-hidden="true" />
          {copy.shortcutsTitle}
        </div>
        <ul className="mt-2.5 grid gap-x-4 gap-y-2 sm:grid-cols-2">
          {DONE_SHORTCUTS.map(([id, text]) => (
            <li key={id} className="flex items-center gap-2 text-caption text-text-main">
              <Keys accelerator={shortcutById(id).accelerator} mac={mac} />
              <span className="min-w-0">{copy[text]}</span>
            </li>
          ))}
        </ul>
      </div>

      {/* Back waits for the downloads: returning and re-entering would start a second download of the same file. */}
      <div className="mt-4 flex items-center justify-between gap-2">
        <SecondaryButton onClick={onBack} disabled={!isReady}>
          {assetsCopy.back}
        </SecondaryButton>
        <PrimaryButton onClick={onComplete} disabled={!isReady}>
          {isReady ? copy.enter : assetsCopy.preparing}
        </PrimaryButton>
      </div>
    </div>
  )
}
