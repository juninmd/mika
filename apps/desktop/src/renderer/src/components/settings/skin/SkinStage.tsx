import { motion } from 'framer-motion'
import { Loader2, Rotate3d, Sparkles } from 'lucide-react'
import { lazy, Suspense, useEffect, useState } from 'react'
import { useI18n } from '../../../hooks/useI18n'
import type { Live2DModelOption } from '../../live2d/model-path'
import { StatusPill } from '../ds'
import { SkinArt } from './SkinArt'
import type { SkinFacts } from './skin-catalog'
import type { ShowcaseStatus } from './VrmShowcase'

// three.js + the VRM rig only load when the Skin tab actually shows a 3D stage.
const VrmShowcase = lazy(() => import('./VrmShowcase'))

/** Arrowing through the roster must not start a 10–30 MB model parse per keypress. */
const STAGE_DEBOUNCE_MS = 320

function useSettled<T>(value: T, delayMs: number): T {
  const [settled, setSettled] = useState(value)
  useEffect(() => {
    const id = window.setTimeout(() => setSettled(value), delayMs)
    return () => window.clearTimeout(id)
  }, [value, delayMs])
  return settled
}

type Props = {
  model: Live2DModelOption
  label: string
  facts: SkinFacts
}

function StageBackdrop() {
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_70%_55%_at_50%_38%,var(--accent-glow),transparent_70%)]" />
      <div className="absolute inset-x-0 bottom-0 h-2/5 bg-gradient-to-t from-surface-sunken to-transparent" />
      <div className="absolute inset-x-[12%] bottom-[9%] h-px bg-gradient-to-r from-transparent via-accent-rim to-transparent" />
      <div className="absolute inset-0 bg-[repeating-linear-gradient(0deg,transparent_0_3px,var(--surface-raise)_3px_4px)] opacity-40 mix-blend-overlay" />
    </div>
  )
}

function Live2DStage({ model, label }: { model: Live2DModelOption; label: string }) {
  return (
    <motion.div
      key={model.id}
      className="absolute inset-x-6 bottom-8 top-6 flex items-end justify-center"
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: [0, -6, 0] }}
      transition={{
        opacity: { duration: 0.2 },
        y: { duration: 4, repeat: Number.POSITIVE_INFINITY, ease: 'easeInOut' },
      }}
    >
      <SkinArt model={model} label={label} className="h-full w-full object-contain object-bottom" />
    </motion.div>
  )
}

function VrmStage({ model, label }: { model: Live2DModelOption; label: string }) {
  const path = useSettled(model.path, STAGE_DEBOUNCE_MS)
  const [status, setStatus] = useState<ShowcaseStatus>('loading')
  const [specialMove, setSpecialMove] = useState(0)
  const { t } = useI18n()
  const pending = status === 'loading' || path !== model.path
  return (
    <>
      {/* Poster while the model parses — the same art the roster card shows, never an empty stage. */}
      {(pending || status === 'error') && (
        <SkinArt
          model={model}
          label={label}
          className="absolute inset-x-8 bottom-10 top-6 h-auto w-auto object-contain object-bottom opacity-60 blur-[1px]"
        />
      )}
      <Suspense fallback={null}>
        <VrmShowcase key={path} modelPath={path} onStatus={setStatus} specialMove={specialMove} />
      </Suspense>
      {status === 'ready' && !pending && (
        <button
          type="button"
          onClick={() => setSpecialMove((n) => n + 1)}
          className="absolute left-4 top-4 z-10 inline-flex items-center gap-1.5 rounded-full border border-accent-rim bg-bg-surface/80 px-3 py-1.5 text-caption font-black uppercase tracking-[0.12em] text-accent-text shadow-[0_0_18px_var(--accent-glow)] backdrop-blur-sm transition-transform duration-150 hover:-translate-y-0.5 active:scale-[0.97] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus motion-reduce:transition-none motion-reduce:hover:translate-y-0"
        >
          <Sparkles size={12} aria-hidden="true" />
          {t('settings.skin.stage.specialMove')}
        </button>
      )}
      {pending && (
        <span className="absolute right-4 top-4 flex items-center gap-1.5 rounded-full border border-border-subtle bg-bg-surface/80 px-2.5 py-1 text-caption text-text-muted backdrop-blur-sm">
          <Loader2 size={12} className="animate-spin" aria-hidden="true" />
          {t('settings.skin.stage.loading')}
        </span>
      )}
      {status === 'error' && !pending && (
        <span className="absolute inset-x-6 top-4 rounded-xl border border-danger-rim bg-danger-soft px-3 py-2 text-caption text-danger">
          {t('settings.skin.stage.error')}
        </span>
      )}
    </>
  )
}

export function SkinStage({ model, label, facts }: Props) {
  const { t } = useI18n()
  const isVrm = facts.kind === 'vrm'
  return (
    <figure
      aria-label={t('settings.skin.stage.ariaLabel').replace('{{name}}', label)}
      className="relative isolate h-[500px] overflow-hidden rounded-3xl border border-border-subtle bg-bg-deep shadow-[inset_0_1px_0_var(--surface-raise)]"
    >
      <StageBackdrop />
      {isVrm ? <VrmStage model={model} label={label} /> : <Live2DStage model={model} label={label} />}
      <figcaption className="pointer-events-none absolute inset-x-0 bottom-0 flex flex-wrap items-end justify-between gap-x-3 gap-y-1 bg-gradient-to-t from-bg-deep via-bg-deep/70 to-transparent px-5 pb-4 pt-16">
        <div className="min-w-0">
          <p className="text-micro font-black uppercase tracking-[0.12em] text-accent-text">
            {t(isVrm ? 'settings.skin.kind.vrm' : 'settings.skin.kind.live2d')}
          </p>
          <p className="truncate text-3xl font-black tracking-tight text-text-main">{label}</p>
        </div>
        {isVrm ? (
          <span className="flex shrink-0 items-center gap-1.5 text-caption text-text-muted">
            <Rotate3d size={14} aria-hidden="true" />
            {t('settings.skin.stage.dragHint')}
          </span>
        ) : (
          !model.bundled && <StatusPill variant="warn" label={t('settings.skin.modelStation.thirdPartyBadge')} />
        )}
      </figcaption>
    </figure>
  )
}
