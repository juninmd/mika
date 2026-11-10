import { GlassCard } from '@mika/sdk-ui/settings'
import {
  BookOpenText,
  BrainCircuit,
  Camera,
  ClipboardList,
  Code2,
  Gamepad2,
  Gauge,
  MessageSquareText,
  ScanFace,
} from 'lucide-react'
import { labelOf } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import { formatInterval, Metric, ReadinessPill } from './vision-mission-parts'
import type { VisionPrivacy } from './vision-readiness'

type Props = {
  provider: string
  analysisMode: string
  captureIntervalMs: number
  proactiveCommentaryChance: number
  codeAwareness: boolean
  gameAwareness: boolean
  readClipboardImages: boolean
  visualDiaryEnabled: boolean
  visionAssistAutoComment: boolean
  privacy: VisionPrivacy
}

export function VisionMissionPanel({
  provider,
  analysisMode,
  captureIntervalMs,
  proactiveCommentaryChance,
  codeAwareness,
  gameAwareness,
  readClipboardImages,
  visualDiaryEnabled,
  visionAssistAutoComment,
  privacy,
}: Props) {
  const { t } = useI18n()

  const providerOptions = [
    { label: t('settings.vision.field.provider.option.default'), value: 'default' },
    { label: t('settings.vision.field.provider.option.openrouter'), value: 'openrouter' },
    { label: t('settings.vision.field.provider.option.google'), value: 'google' },
    { label: t('settings.vision.field.provider.option.ollama'), value: 'ollama' },
    { label: t('settings.vision.field.provider.option.openaiCompatible'), value: 'openaiCompatible' },
    { label: t('settings.vision.field.provider.option.unsloth'), value: 'unsloth' },
  ]

  const analysisModeOptions = [
    { label: t('settings.vision.field.analysisMode.option.focused'), value: 'focused' },
    { label: t('settings.vision.field.analysisMode.option.balanced'), value: 'balanced' },
    { label: t('settings.vision.field.analysisMode.option.full'), value: 'full' },
  ]

  const loopItems = [
    {
      label: t('settings.vision.mission.loop.capture.label'),
      detail: t('settings.vision.mission.loop.capture.detail'),
      icon: Camera,
      color: 'text-info',
    },
    {
      label: t('settings.vision.mission.loop.interpret.label'),
      detail: t('settings.vision.mission.loop.interpret.detail'),
      icon: BrainCircuit,
      color: 'text-accent',
    },
    {
      label: t('settings.vision.mission.loop.comment.label'),
      detail: t('settings.vision.mission.loop.comment.detail'),
      icon: MessageSquareText,
      color: 'text-ok',
    },
    {
      label: t('settings.vision.mission.loop.catalog.label'),
      detail: t('settings.vision.mission.loop.catalog.detail'),
      icon: ScanFace,
      color: 'text-warn',
    },
  ]

  // Games/code/diary read the screen; clipboard reading needs its own consent. Enabled-but-gated
  // shows amber, not ATIVO — the Rust side refuses the capture either way.
  const screenBlocked = !privacy.screenCapture
  const readiness = [
    { label: t('settings.vision.mission.readiness.games'), on: gameAwareness, blocked: screenBlocked, icon: Gamepad2 },
    { label: t('settings.vision.mission.readiness.code'), on: codeAwareness, blocked: screenBlocked, icon: Code2 },
    {
      label: t('settings.vision.mission.readiness.clipboard'),
      on: readClipboardImages,
      blocked: !privacy.clipboardLogging,
      icon: ClipboardList,
    },
    {
      label: t('settings.vision.mission.readiness.diary'),
      on: visualDiaryEnabled,
      blocked: screenBlocked,
      icon: BookOpenText,
    },
  ]

  return (
    <GlassCard span="full" accent>
      <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p className="text-caption font-black uppercase tracking-[0.16em] text-info">
                {t('settings.vision.mission.kicker')}
              </p>
              <h4 className="mt-1 text-base font-semibold text-text-main">{t('settings.vision.mission.title')}</h4>
            </div>
            <div className="flex items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2">
              <Gauge size={14} className="text-text-muted" />
              <span className="font-mono text-caption text-text-main">{formatInterval(captureIntervalMs)}</span>
            </div>
          </div>

          <div className="grid flex-1 auto-rows-fr gap-2 sm:grid-cols-2">
            {loopItems.map((item) => {
              const Icon = item.icon
              return (
                <div
                  key={item.label}
                  className="flex flex-col justify-center rounded-lg border border-border-subtle bg-surface-raise p-3.5"
                >
                  <div className="flex items-center gap-2.5">
                    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-bg-surface">
                      <Icon size={15} className={item.color} aria-hidden="true" />
                    </span>
                    <span className="text-sm font-semibold text-text-main">{item.label}</span>
                  </div>
                  <p className="mt-2 text-caption leading-snug text-text-muted">{item.detail}</p>
                </div>
              )
            })}
          </div>
        </div>

        <div className="rounded-lg border border-border-subtle bg-surface-raise p-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-caption font-black uppercase tracking-[0.14em] text-warn">
                {t('settings.vision.mission.catalogWarning.title')}
              </p>
              <p className="mt-1 text-caption leading-relaxed text-text-muted">
                {t('settings.vision.mission.catalogWarning.description')}
              </p>
            </div>
            <ScanFace size={18} className="shrink-0 text-warn" />
          </div>

          <div className="mt-3 grid grid-cols-2 gap-2">
            <Metric label={t('settings.vision.mission.metric.provider')} value={labelOf(providerOptions, provider)} />
            <Metric
              label={t('settings.vision.mission.metric.mode')}
              value={labelOf(analysisModeOptions, analysisMode)}
            />
            <Metric
              label={t('settings.vision.mission.metric.comment')}
              value={
                visionAssistAutoComment
                  ? t('settings.vision.mission.comment.spontaneous')
                  : t('settings.vision.mission.comment.manual')
              }
            />
            <Metric
              label={t('settings.vision.mission.metric.chance')}
              value={`${Math.round(proactiveCommentaryChance * 100)}%`}
            />
          </div>

          <div className="mt-3 grid grid-cols-2 gap-2">
            {readiness.map((item) => (
              <ReadinessPill key={item.label} {...item} t={t} />
            ))}
          </div>
        </div>
      </div>
    </GlassCard>
  )
}
