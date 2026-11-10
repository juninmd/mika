import { Check, Download, FileBadge2, Loader2, Smile, Sparkles, Wand2 } from 'lucide-react'
import { useI18n } from '../../../hooks/useI18n'
import type { Live2DModelOption } from '../../live2d/model-path'
import { StatTile, StatusPill } from '../ds'
import { type SkinFacts, SOURCE_KEYS } from './skin-catalog'

type Props = {
  model: Live2DModelOption
  label: string
  facts: SkinFacts
  applied: boolean
  applying: boolean
  onApply: () => void
}

function ApplyButton({ applied, applying, onApply }: Pick<Props, 'applied' | 'applying' | 'onApply'>) {
  const { t } = useI18n()
  const Icon = applying ? Loader2 : applied ? Check : Wand2
  return (
    <button
      type="button"
      onClick={onApply}
      disabled={applied || applying}
      aria-busy={applying}
      className="flex w-full items-center justify-center gap-2 rounded-xl border border-accent-rim bg-accent px-4 py-3 text-sm font-black uppercase tracking-[0.12em] text-bg-deep shadow-[0_0_24px_var(--accent-glow)] transition-[transform,opacity,box-shadow] duration-150 enabled:hover:-translate-y-0.5 enabled:active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-default disabled:border-border-subtle disabled:bg-surface-raise disabled:text-text-muted disabled:shadow-none motion-reduce:transition-none motion-reduce:enabled:hover:translate-y-0"
    >
      <Icon size={16} className={applying ? 'animate-spin' : ''} aria-hidden="true" />
      {t(applied ? 'settings.skin.details.applied' : 'settings.skin.details.apply')}
    </button>
  )
}

function FacePresets({ presets }: { presets: readonly string[] }) {
  const { t } = useI18n()
  if (presets.length === 0) return null
  return (
    <div className="space-y-1.5">
      <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.skin.details.facePresets')}
      </p>
      <div className="flex flex-wrap gap-1.5">
        {presets.map((preset) => (
          <span
            key={preset}
            className="rounded-full border border-border-subtle bg-surface-raise px-2 py-0.5 text-caption text-text-main"
          >
            {t(`settings.skin.details.face.${preset}` as Parameters<typeof t>[0])}
          </span>
        ))}
      </div>
    </div>
  )
}

// What the previewed skin is before committing to it: type, what it can express, where it came from.
export function SkinDetails({ model, label, facts, applied, applying, onApply }: Props) {
  const { t } = useI18n()
  const source = SOURCE_KEYS[facts.source]
  return (
    <div className="flex h-full flex-col gap-4 rounded-2xl border border-border-subtle bg-glass p-4">
      <div className="space-y-2">
        <div className="flex flex-wrap gap-1.5">
          <StatusPill
            variant="accent"
            label={t(facts.kind === 'vrm' ? 'settings.skin.kind.vrm' : 'settings.skin.kind.live2d')}
          />
          {model.bundled ? (
            <StatusPill variant="ok" label={t('settings.skin.details.bundled')} dot />
          ) : (
            <StatusPill variant="warn" label={t('settings.skin.details.downloadNeeded')} dot />
          )}
        </div>
        <h4 className="truncate text-xl font-black tracking-tight text-text-main">{label}</h4>
      </div>
      <div className="grid grid-cols-2 gap-2">
        {/* min-w-0: the tile's own 140px floor overflowed the half-width column of the wizard's narrow card. */}
        <StatTile
          label={t('settings.skin.details.expressions')}
          value={facts.expressions}
          icon={Smile}
          tone="accent"
          className="min-w-0"
        />
        <StatTile
          label={t('settings.skin.details.actions')}
          value={facts.actions}
          icon={Sparkles}
          className="min-w-0"
        />
      </div>
      <FacePresets presets={facts.facePresets} />
      <div className="space-y-1 rounded-xl border border-border-subtle bg-surface-sunken p-3">
        <p className="flex items-center gap-1.5 text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
          <FileBadge2 size={12} className="text-accent" aria-hidden="true" />
          {t('settings.skin.details.sourceLabel')}
        </p>
        <p className="text-caption leading-snug text-text-main">{t(source.origin)}</p>
        <p className="text-caption leading-snug text-text-muted">{t(source.license)}</p>
      </div>
      {!model.bundled && (
        <p className="flex gap-2 rounded-xl border border-warn-rim bg-warn-soft p-3 text-caption leading-snug text-warn">
          <Download size={14} className="mt-px shrink-0" aria-hidden="true" />
          {t('settings.skin.details.downloadHint')}
        </p>
      )}
      <div className="mt-auto">
        <ApplyButton applied={applied} applying={applying} onApply={onApply} />
      </div>
    </div>
  )
}
