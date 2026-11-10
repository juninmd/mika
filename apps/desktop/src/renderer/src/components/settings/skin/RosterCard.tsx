import { PRESSABLE, SPRING } from '@mika/sdk-ui/settings'
import { motion, useMotionValue, useReducedMotion, useSpring, useTransform } from 'framer-motion'
import { Check, Download, Loader2 } from 'lucide-react'
import type { KeyboardEvent, PointerEvent, Ref } from 'react'
import { useI18n } from '../../../hooks/useI18n'
import type { Live2DModelOption } from '../../live2d/model-path'
import { SkinArt } from './SkinArt'
import type { SkinKind } from './skin-catalog'

/** Max hover tilt, in degrees, and how far (px) the art drifts against it for depth. */
const TILT_DEG = 7
const PARALLAX_PX = 6

// Category frame: bundled 3D skins wear the theme accent; third-party 2D samples a warning hue.
const FRAME: Record<SkinKind, string> = {
  vrm: 'from-accent via-accent-rim to-info-rim',
  live2d: 'from-warn via-warn-rim to-border-strong',
}

type Props = {
  model: Live2DModelOption
  label: string
  kind: SkinKind
  previewed: boolean
  applied: boolean
  applying: boolean
  cardRef: Ref<HTMLButtonElement>
  onPreview: () => void
  onApply: () => void
  onKeyDown: (e: KeyboardEvent<HTMLButtonElement>) => void
}

function useTilt() {
  const reduce = useReducedMotion()
  const px = useMotionValue(0)
  const py = useMotionValue(0)
  const sx = useSpring(px, SPRING.soft)
  const sy = useSpring(py, SPRING.soft)
  const rotateY = useTransform(sx, [-0.5, 0.5], [-TILT_DEG, TILT_DEG])
  const rotateX = useTransform(sy, [-0.5, 0.5], [TILT_DEG, -TILT_DEG])
  const artX = useTransform(sx, [-0.5, 0.5], [PARALLAX_PX, -PARALLAX_PX])
  const artY = useTransform(sy, [-0.5, 0.5], [PARALLAX_PX, -PARALLAX_PX])
  const move = (e: PointerEvent<HTMLElement>) => {
    if (reduce) return
    const rect = e.currentTarget.getBoundingClientRect()
    px.set((e.clientX - rect.left) / rect.width - 0.5)
    py.set((e.clientY - rect.top) / rect.height - 0.5)
  }
  const leave = () => {
    px.set(0)
    py.set(0)
  }
  return { card: { rotateX, rotateY, transformPerspective: 700 }, art: { x: artX, y: artY }, move, leave }
}

function CardBadges({ model, applied, applying }: Pick<Props, 'model' | 'applied' | 'applying'>) {
  const { t } = useI18n()
  return (
    <>
      {!model.bundled && (
        <span
          className="absolute left-2 top-2 flex items-center gap-1 rounded-full border border-warn-rim bg-bg-surface/85 px-2 py-0.5 text-micro font-black uppercase tracking-[0.12em] text-warn backdrop-blur-sm"
          title={t('settings.skin.modelStation.thirdPartyTooltip')}
        >
          <Download size={10} aria-hidden="true" />
          {t('settings.skin.modelStation.thirdPartyBadge')}
        </span>
      )}
      {applied && !applying && (
        <span className="absolute right-2 top-2 flex items-center gap-1 rounded-full bg-accent px-2 py-0.5 text-micro font-black uppercase tracking-[0.12em] text-bg-deep shadow-[0_0_12px_var(--accent-glow)]">
          <Check size={10} strokeWidth={3} aria-hidden="true" />
          {t('settings.skin.roster.inUse')}
        </span>
      )}
      {applying && (
        <span className="absolute inset-0 flex items-center justify-center bg-surface-sunken text-accent-text">
          <Loader2 size={18} className="animate-spin" aria-hidden="true" />
        </span>
      )}
    </>
  )
}

// One roster slot: bust art in a category-gradient frame, name plate, hover tilt with parallax.
// Click/arrows preview the skin on the stage; double-click or Enter applies it (same as "Aplicar").
export function RosterCard(props: Props) {
  const { model, label, kind, previewed, applied, applying, cardRef, onPreview, onApply, onKeyDown } = props
  const tilt = useTilt()
  const { t } = useI18n()
  return (
    <motion.button
      ref={cardRef}
      type="button"
      role="radio"
      aria-checked={previewed}
      aria-busy={applying}
      aria-label={applied ? `${label} — ${t('settings.skin.roster.inUse')}` : label}
      tabIndex={previewed ? 0 : -1}
      onClick={onPreview}
      onDoubleClick={onApply}
      onKeyDown={onKeyDown}
      onPointerMove={tilt.move}
      onPointerLeave={tilt.leave}
      whileHover={PRESSABLE.whileHover}
      whileTap={PRESSABLE.whileTap}
      transition={PRESSABLE.transition}
      style={tilt.card}
      className={`group relative block rounded-2xl bg-gradient-to-br p-px text-left transition-[opacity,box-shadow] duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-deep motion-reduce:transition-none ${FRAME[kind]} ${
        previewed ? 'opacity-100 shadow-[0_0_24px_var(--accent-glow)]' : 'opacity-70 hover:opacity-100'
      }`}
    >
      <span className="relative block aspect-[4/5] overflow-hidden rounded-[15px] bg-surface-sunken">
        <span className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_20%,var(--accent-glow),transparent_65%)]" />
        <motion.span className="absolute -inset-2 block" style={tilt.art}>
          <span className="block h-full w-full origin-top" style={{ scale: model.previewZoom ?? 1 }}>
            <SkinArt model={model} label={label} className="h-full w-full object-cover object-top" />
          </span>
        </motion.span>
        <span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-bg-deep via-bg-deep/80 to-transparent px-2.5 pb-2 pt-8">
          <span className="block truncate text-sm font-bold text-text-main">{label}</span>
          <span className="block text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t(kind === 'vrm' ? 'settings.skin.kind.vrmShort' : 'settings.skin.kind.live2dShort')}
          </span>
        </span>
        <CardBadges model={model} applied={applied} applying={applying} />
        {previewed && (
          <span className="pointer-events-none absolute inset-0 rounded-[15px] ring-2 ring-inset ring-accent" />
        )}
      </span>
    </motion.button>
  )
}
