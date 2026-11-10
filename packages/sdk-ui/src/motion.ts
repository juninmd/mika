import type { Transition, Variants } from 'framer-motion'

/**
 * Sistema de movimento compartilhado: uma régua de duração/curva/mola para toda a UI.
 *
 * Antes cada componente escolhia a sua (0.15, 0.18, 0.2, 0.25, 0.3 s; `easeOut`, cubic-bezier
 * genérico, mola 500/30) e a mesma troca de aba animava duas vezes em sequência. Regras:
 * - só `transform`/`opacity` (compositor) — nada de `height`/`width`/`top` salvo colapso explícito;
 * - entradas curtas (≤ 220 ms) e saídas mais curtas ainda: a UI responde primeiro e enfeita depois;
 * - reduced motion é tratado uma vez no `MotionConfig reducedMotion="user"` da raiz.
 */
export const DURATION = {
  /** Feedback de toque (press, hover de ícone). */
  instant: 0.08,
  /** Saídas, dropdowns, tooltips. */
  fast: 0.14,
  /** Entradas de conteúdo, troca de página. */
  base: 0.2,
  /** Colapsos de altura, elementos grandes. */
  slow: 0.28,
} as const

/** Curvas cubic-bezier. `out` desacelera forte: começa rápido, parece imediato. */
export const EASE = {
  out: [0.22, 1, 0.36, 1],
  inOut: [0.65, 0, 0.35, 1],
  in: [0.4, 0, 1, 1],
} as const satisfies Record<string, readonly [number, number, number, number]>

/** Molas: `snappy` para interruptores/seleção, `soft` para painéis e bolhas, `pop` para ênfase. */
export const SPRING = {
  snappy: { type: 'spring', stiffness: 700, damping: 40, mass: 0.6 },
  soft: { type: 'spring', stiffness: 380, damping: 34, mass: 0.8 },
  pop: { type: 'spring', stiffness: 520, damping: 26, mass: 0.7 },
} as const satisfies Record<string, Transition>

const STAGGER_STEP = 0.03
const STAGGER_MAX_ITEMS = 8

/** Atraso de entrada escalonado, com teto: a 40ª linha não pode esperar 1,2 s para aparecer. */
export function staggerDelay(index: number, step = STAGGER_STEP): number {
  const i = Number.isFinite(index) && index > 0 ? Math.floor(index) : 0
  return Math.min(i, STAGGER_MAX_ITEMS) * step
}

const enter = (delay = 0): Transition => ({ duration: DURATION.base, ease: EASE.out, delay })
const exit: Transition = { duration: DURATION.fast, ease: EASE.in }

/**
 * Presets prontos para espalhar num `motion.*` (`<motion.div {...MOTION.fadeUp}>`). Cada um anima
 * uma vez na montagem; re-render com os mesmos valores não reanima.
 */
export const MOTION = {
  fadeIn: {
    initial: { opacity: 0 },
    animate: { opacity: 1 },
    exit: { opacity: 0, transition: exit },
    transition: enter(),
  },
  fadeUp: {
    initial: { opacity: 0, y: 6 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: -4, transition: exit },
    transition: enter(),
  },
  /** Troca de aba/página: entrada curta, sem saída (a nova tela não espera a velha sair). */
  page: {
    initial: { opacity: 0, y: 4 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: DURATION.base, ease: EASE.out },
  },
  /** Dropdown/popover ancorado em cima. */
  popover: {
    initial: { opacity: 0, y: -4, scale: 0.98 },
    animate: { opacity: 1, y: 0, scale: 1 },
    exit: { opacity: 0, y: -2, scale: 0.99, transition: exit },
    transition: { duration: DURATION.fast, ease: EASE.out },
  },
  /** Diálogo/paleta central. */
  dialog: {
    initial: { opacity: 0, y: 8, scale: 0.98 },
    animate: { opacity: 1, y: 0, scale: 1 },
    exit: { opacity: 0, y: 4, scale: 0.99, transition: exit },
    transition: SPRING.soft,
  },
  /** Toast que entra pela lateral direita. */
  toast: {
    initial: { opacity: 0, x: 24, scale: 0.98 },
    animate: { opacity: 1, x: 0, scale: 1 },
    exit: { opacity: 0, x: 16, scale: 0.98, transition: exit },
    transition: SPRING.soft,
  },
  /** Bolha de chat/fala: sobe e cresce a partir da base. */
  bubble: {
    initial: { opacity: 0, y: 8, scale: 0.97 },
    animate: { opacity: 1, y: 0, scale: 1 },
    exit: { opacity: 0, y: 4, scale: 0.98, transition: exit },
    transition: SPRING.soft,
  },
  /** Painel que desliza de baixo (dock/composer). */
  sheet: {
    initial: { opacity: 0, y: 16 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: 10, transition: exit },
    transition: SPRING.soft,
  },
} as const

/** Hover/press de card clicável: sobe 2 px e afunda no toque, mola curta. */
export const PRESSABLE = {
  whileHover: { y: -2 },
  whileTap: { scale: 0.985, y: 0 },
  transition: SPRING.snappy,
} as const

/**
 * Revelação escalonada de seções. O contêiner (a página) usa `variants={REVEAL.group}` com `initial="hidden"
 * animate="show"`; cada filho com `variants={REVEAL.item}` herda os rótulos por contexto do
 * framer-motion, sem precisar ser filho DOM direto — por isso o `GlassSection` entra escalonado
 * dentro de qualquer página de configurações e aparece estático fora de um grupo.
 */
export const REVEAL = {
  /** Contêiner de página: fade curto e escalona os `item` descendentes. */
  page: {
    hidden: { opacity: 0 },
    show: {
      opacity: 1,
      transition: { duration: DURATION.fast, ease: EASE.out, staggerChildren: STAGGER_STEP, delayChildren: 0.02 },
    },
  },
  group: {
    hidden: {},
    show: { transition: { staggerChildren: STAGGER_STEP, delayChildren: 0.02 } },
  },
  item: {
    hidden: { opacity: 0, y: 8 },
    show: { opacity: 1, y: 0, transition: { duration: DURATION.base, ease: EASE.out } },
  },
} as const satisfies Record<string, Variants>

/**
 * Classes Tailwind para transição CSS de controles (sem framer): só propriedades de compositor e
 * de pintura, nunca `transition` (que também interpola padding/width e dispara layout).
 */
export const MOTION_CLASS = {
  interactive:
    'transition-[transform,opacity,background-color,border-color,color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none',
  press: 'active:scale-[0.97] motion-reduce:active:scale-100',
  lift: 'enabled:hover:-translate-y-0.5 motion-reduce:enabled:hover:translate-y-0',
} as const
