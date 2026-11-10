import { choiceOption, GlassCard, GlassSlider, GlassToggle, type ProviderChoice } from '@mika/sdk-ui/settings'
import {
  Code2,
  Gamepad2,
  Globe,
  Link2,
  MessageSquare,
  Plug,
  Scale,
  ScanEye,
  Server,
  Sparkles,
  Target,
  Zap,
} from 'lucide-react'
import { type BadgeKind, providerBadges } from '../../src/components/settings/provider-badges'
import type { Translate } from '../../src/hooks/useI18n'
import { useI18n } from '../../src/hooks/useI18n'

const PROVIDER_LOOK: Record<string, { icon: ProviderChoice['icon']; badges: BadgeKind[] }> = {
  default: { icon: Link2, badges: ['recommended'] },
  openrouter: { icon: Globe, badges: ['cloud', 'apiKey'] },
  google: { icon: Sparkles, badges: ['cloud', 'apiKey'] },
  ollama: { icon: Server, badges: ['local', 'gpu'] },
  openaiCompatible: { icon: Plug, badges: ['apiKey'] },
  unsloth: { icon: Zap, badges: ['local', 'apiKey'] },
}

export function getProviderChoices(t: Translate): ProviderChoice[] {
  const ids = ['default', 'openrouter', 'google', 'ollama', 'openaiCompatible', 'unsloth'] as const
  return ids.map((id) => ({
    id,
    label: t(`settings.vision.field.provider.option.${id}`),
    desc: t(`settings.vision.provider.${id}.desc`),
    icon: PROVIDER_LOOK[id]?.icon,
    badges: providerBadges(t, ...(PROVIDER_LOOK[id]?.badges ?? [])),
  }))
}

export function getAnalysisModes(t: Translate) {
  return [
    choiceOption(
      t('settings.vision.field.analysisMode.option.focused'),
      'focused',
      t('settings.vision.components.analysisMode.focused.description'),
      Target,
    ),
    choiceOption(
      t('settings.vision.field.analysisMode.option.balanced'),
      'balanced',
      t('settings.vision.components.analysisMode.balanced.description'),
      Scale,
    ),
    choiceOption(
      t('settings.vision.field.analysisMode.option.full'),
      'full',
      t('settings.vision.components.analysisMode.full.description'),
      ScanEye,
    ),
  ]
}

export type SliderControl = {
  label: string
  configPath: string
  value: number
  min: number
  max: number
  step?: number
  showValue?: (value: number) => string
}

export type ToggleControl = {
  label: string
  description?: string
  configPath: string
  value: boolean
}

export function SliderCard({
  control,
  onSave,
}: {
  control: SliderControl
  onSave: (path: string, value: number) => Promise<void>
}) {
  return (
    <GlassCard>
      <GlassSlider {...control} onSave={onSave} />
    </GlassCard>
  )
}

export function ToggleCard({
  control,
  onSave,
}: {
  control: ToggleControl
  onSave: (path: string, value: boolean) => Promise<void>
}) {
  return (
    <GlassCard>
      <GlassToggle {...control} onSave={onSave} />
    </GlassCard>
  )
}

export function VisionDisabledHero({ onEnable }: { onEnable: () => void }) {
  const { t } = useI18n()

  const offHighlights = [
    {
      icon: Code2,
      title: t('settings.vision.components.disabled.highlight.code.title'),
      detail: t('settings.vision.components.disabled.highlight.code.detail'),
    },
    {
      icon: Gamepad2,
      title: t('settings.vision.components.disabled.highlight.game.title'),
      detail: t('settings.vision.components.disabled.highlight.game.detail'),
    },
    {
      icon: MessageSquare,
      title: t('settings.vision.components.disabled.highlight.comment.title'),
      detail: t('settings.vision.components.disabled.highlight.comment.detail'),
    },
  ]

  return (
    <GlassCard span="full">
      <div className="space-y-5 py-2">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0">
            <p className="text-sm font-semibold text-text-main">{t('settings.vision.components.disabled.title')}</p>
            <p className="mt-1 max-w-xl text-caption leading-relaxed text-text-muted">
              {t('settings.vision.components.disabled.description')}
            </p>
          </div>
          <button
            type="button"
            onClick={onEnable}
            className="shrink-0 self-start rounded-lg border border-accent-rim bg-accent-soft px-4 py-2 text-xs font-semibold text-accent transition-colors hover:bg-accent/25 sm:self-auto"
          >
            {t('settings.vision.components.disabled.enableButton')}
          </button>
        </div>

        <div className="grid gap-3 sm:grid-cols-3">
          {offHighlights.map(({ icon: Icon, title, detail }) => (
            <div key={title} className="rounded-lg border border-border-subtle bg-surface-raise p-3">
              <div className="flex items-center gap-2">
                <Icon size={13} style={{ color: 'var(--accent)' }} />
                <p className="text-caption font-semibold text-text-main">{title}</p>
              </div>
              <p className="mt-1.5 text-caption leading-relaxed text-text-muted">{detail}</p>
            </div>
          ))}
        </div>

        <p className="text-caption leading-relaxed text-text-subtle">
          {t('settings.vision.components.disabled.footer')}
        </p>
      </div>
    </GlassCard>
  )
}
